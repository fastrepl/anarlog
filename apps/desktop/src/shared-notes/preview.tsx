import { convertFileSrc } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";

import type { JSONContent } from "@hypr/editor/note";
import { commands as attachmentSyncCommands } from "@hypr/plugin-attachment-sync";

import type { SharedAttachmentDownload } from "./attachment-client";
import type { SharedNoteAttachment } from "./cache";

import { attachmentTransferNative } from "~/attachment-sync/native";
import { useAuth } from "~/auth";
import { env } from "~/env";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

const MAX_PREVIEW_BODY_BYTES = 2 * 1024 * 1024;
const MAX_PREVIEW_RESPONSE_BYTES = MAX_PREVIEW_BODY_BYTES + 256 * 1024;
const MAX_ATTACHMENT_GRANT_RESPONSE_BYTES = 16 * 1024;
const MAX_PREVIEW_TITLE_BYTES = 4096;
const MAX_PREVIEW_DEPTH = 64;
const MAX_PREVIEW_NODES = 50_000;
const MAX_PREVIEW_ATTACHMENTS = 64;
const MAX_PREVIEW_ATTACHMENT_BYTES = 512 * 1024 * 1024;
const TRANSIENT_RETRY_ATTEMPTS = 3;
const TRANSIENT_RETRY_DELAY_MS = 100;
const RATE_LIMIT_RETRY_DELAY_MS = 1000;
const MAX_RETRY_DELAY_MS = 5000;
const PREVIEW_ATTACHMENT_SCOPE_PREFIX = "preview:";
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PREVIEW_KEYS = new Set([
  "shareId",
  "schemaVersion",
  "contentRevision",
  "title",
  "body",
  "attachments",
  "publishedAt",
]);
const HANDOFF_CLAIM_KEYS = new Set([...PREVIEW_KEYS, "leaseExpiresAt"]);

class NonRetryableSharedNoteError extends Error {}

class TransientSharedNoteError extends Error {
  constructor(
    message: string,
    readonly retryAfterMs: number,
  ) {
    super(message);
  }
}

export type SharedNotePreviewSnapshot = {
  shareId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: JSONContent;
  attachments: SharedNoteAttachment[];
  attachmentDownloads: Array<
    SharedAttachmentDownload & { localPath?: string; localSrc?: string }
  >;
  publishedAt: string;
};

export type SharedNotePreviewState =
  | { status: "loading" }
  | { status: "ready"; snapshot: SharedNotePreviewSnapshot }
  | { status: "unavailable" };

type PreviewClaimResult = {
  snapshot: SharedNotePreviewSnapshot;
  downloadAttachment: (
    attachment: SharedNoteAttachment,
    signal: AbortSignal,
  ) => Promise<SharedAttachmentDownload>;
};

type PreviewClaim = (signal: AbortSignal) => Promise<PreviewClaimResult>;

const unavailableState = { status: "unavailable" } as const;
const states = new Map<string, SharedNotePreviewState>();
const controllers = new Map<string, AbortController>();
const attachmentCacheTasks = new Map<string, Promise<void>>();
const listeners = new Set<() => void>();

export function beginSharedNotePreview(
  claim: PreviewClaim,
  createViewId: () => string = () => crypto.randomUUID(),
) {
  const viewId = createViewId();
  if (
    !UUID_PATTERN.test(viewId) ||
    states.has(viewId) ||
    attachmentCacheTasks.has(viewId)
  ) {
    throw new Error("shared-note preview unavailable");
  }

  const controller = new AbortController();
  states.set(viewId, { status: "loading" });
  controllers.set(viewId, controller);
  emitChange();

  void claim(controller.signal)
    .then(({ snapshot, downloadAttachment }) => {
      if (controllers.get(viewId) !== controller || controller.signal.aborted) {
        return;
      }
      states.set(viewId, { status: "ready", snapshot });
      emitChange();
      const cacheTask = cachePreviewAttachments(
        viewId,
        snapshot,
        downloadAttachment,
        controller.signal,
      );
      attachmentCacheTasks.set(viewId, cacheTask);
      const forgetCacheTask = () => {
        if (attachmentCacheTasks.get(viewId) === cacheTask) {
          attachmentCacheTasks.delete(viewId);
        }
      };
      void cacheTask.then(forgetCacheTask, forgetCacheTask);
      return cacheTask;
    })
    .catch(() => {
      if (controllers.get(viewId) !== controller || controller.signal.aborted) {
        return;
      }
      states.set(viewId, unavailableState);
      emitChange();
    })
    .finally(() => {
      if (controllers.get(viewId) === controller) {
        controllers.delete(viewId);
      }
    });

  return viewId;
}

export function useSharedNotePreview(viewId: string) {
  return useSyncExternalStore(
    subscribe,
    () => states.get(viewId) ?? unavailableState,
    () => unavailableState,
  );
}

export function purgeSharedNotePreview(viewId: string) {
  controllers.get(viewId)?.abort();
  controllers.delete(viewId);
  const cacheTask = attachmentCacheTasks.get(viewId);
  const deleted = states.delete(viewId);
  if (deleted) {
    emitChange();
  }
  clearPreviewAttachmentScope(viewId, cacheTask);
}

export function purgeAllSharedNotePreviews() {
  const viewIds = new Set([...states.keys(), ...attachmentCacheTasks.keys()]);
  for (const controller of controllers.values()) {
    controller.abort();
  }
  controllers.clear();
  if (states.size > 0) {
    states.clear();
    emitChange();
  }
  for (const viewId of viewIds) {
    clearPreviewAttachmentScope(viewId, attachmentCacheTasks.get(viewId));
  }
}

async function cachePreviewAttachments(
  viewId: string,
  snapshot: SharedNotePreviewSnapshot,
  downloadAttachment: PreviewClaimResult["downloadAttachment"],
  signal: AbortSignal,
) {
  let nextIndex = 0;
  const workerCount = Math.min(4, snapshot.attachments.length);
  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      for (;;) {
        if (signal.aborted) return;
        const index = nextIndex;
        nextIndex += 1;
        const attachment = snapshot.attachments[index];
        if (!attachment) return;
        try {
          const { download, cached } = await withTransientRetries(async () => {
            const download = await downloadAttachment(attachment, signal);
            signal.throwIfAborted();
            const cached =
              await attachmentTransferNative.downloadSharedAttachment(
                {
                  scopeId: previewAttachmentScopeId(viewId),
                  attachmentId: download.id,
                  signedUrl: download.signedUrl,
                  expectedSha256: download.sha256,
                  expectedSizeBytes: download.sizeBytes,
                },
                signal,
              );
            return { download, cached };
          }, signal);
          if (signal.aborted) return;
          const current = states.get(viewId);
          if (
            current?.status !== "ready" ||
            current.snapshot.shareId !== snapshot.shareId ||
            current.snapshot.contentRevision !== snapshot.contentRevision
          ) {
            return;
          }
          states.set(viewId, {
            status: "ready",
            snapshot: {
              ...current.snapshot,
              attachmentDownloads: [
                ...current.snapshot.attachmentDownloads.filter(
                  (candidate) => candidate.id !== download.id,
                ),
                {
                  ...download,
                  localPath: cached.localPath,
                  localSrc: convertFileSrc(cached.localPath),
                },
              ],
            },
          });
          emitChange();
        } catch {
          if (signal.aborted) return;
          // The note remains usable even if an optional attachment cannot be cached.
        }
      }
    }),
  );
}

export async function claimSharedNoteHandoff(
  requestId: string,
  signal: AbortSignal,
  fetcher: typeof fetch = fetch,
  createLeaseId: () => string = () => crypto.randomUUID(),
) {
  if (!UUID_PATTERN.test(requestId)) {
    throw new Error("shared-note handoff unavailable");
  }
  const leaseId = createLeaseId();
  if (!UUID_PATTERN.test(leaseId)) {
    throw new Error("shared-note handoff unavailable");
  }

  const text = await withTransientRetries(async () => {
    const response = await fetcher(
      new URL("/shared-notes/handoffs/claim", env.VITE_API_URL),
      {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ requestId, leaseId }),
        cache: "no-store",
        credentials: "omit",
        redirect: "error",
        signal,
      },
    );
    requireSuccessfulResponse(response, "shared-note handoff unavailable");

    const contentLength = Number(response.headers.get("content-length"));
    if (
      Number.isFinite(contentLength) &&
      contentLength > MAX_PREVIEW_RESPONSE_BYTES
    ) {
      throw new NonRetryableSharedNoteError("shared-note handoff unavailable");
    }
    const responseText = await response.text();
    if (utf8Length(responseText) > MAX_PREVIEW_RESPONSE_BYTES) {
      throw new NonRetryableSharedNoteError("shared-note handoff unavailable");
    }
    return responseText;
  }, signal);

  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("shared-note handoff unavailable");
  }
  if (
    !isRecord(value) ||
    Object.keys(value).some((key) => !HANDOFF_CLAIM_KEYS.has(key)) ||
    typeof value.leaseExpiresAt !== "string" ||
    !Number.isFinite(Date.parse(value.leaseExpiresAt))
  ) {
    throw new Error("shared-note handoff unavailable");
  }
  const snapshotValue = { ...value };
  delete snapshotValue.leaseExpiresAt;
  const snapshot = parseSharedNotePreviewSnapshot(snapshotValue);
  return {
    snapshot,
    downloadAttachment: (
      attachment: SharedNoteAttachment,
      attachmentSignal: AbortSignal,
    ) =>
      downloadHandoffAttachment(leaseId, attachment, attachmentSignal, fetcher),
  };
}

export function parseSharedNotePreviewSnapshot(
  value: unknown,
): SharedNotePreviewSnapshot {
  if (
    !isRecord(value) ||
    Object.keys(value).some((key) => !PREVIEW_KEYS.has(key)) ||
    typeof value.shareId !== "string" ||
    !UUID_PATTERN.test(value.shareId) ||
    value.schemaVersion !== 1 ||
    !Number.isSafeInteger(value.contentRevision) ||
    (value.contentRevision as number) < 1 ||
    typeof value.title !== "string" ||
    value.title.trim() !== value.title ||
    utf8Length(value.title) > MAX_PREVIEW_TITLE_BYTES ||
    typeof value.publishedAt !== "string" ||
    !Number.isFinite(Date.parse(value.publishedAt)) ||
    !isRecord(value.body) ||
    value.body.type !== "doc"
  ) {
    throw new Error("invalid shared-note preview snapshot");
  }
  validatePreviewDocument(value.body);
  let encodedBody: string;
  try {
    encodedBody = JSON.stringify(value.body);
  } catch {
    throw new Error("invalid shared-note preview snapshot");
  }
  if (utf8Length(encodedBody) > MAX_PREVIEW_BODY_BYTES) {
    throw new Error("invalid shared-note preview snapshot");
  }

  return {
    shareId: value.shareId,
    schemaVersion: 1,
    contentRevision: value.contentRevision as number,
    title: value.title,
    body: value.body as JSONContent,
    attachments: parsePreviewAttachments(value.attachments ?? [], [])
      .attachments,
    attachmentDownloads: [],
    publishedAt: value.publishedAt,
  };
}

async function downloadHandoffAttachment(
  leaseId: string,
  attachment: SharedNoteAttachment,
  signal: AbortSignal,
  fetcher: typeof fetch,
) {
  const response = await fetcher(
    new URL(
      `/shared-notes/handoffs/attachments/${attachment.id}/download`,
      env.VITE_API_URL,
    ),
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ leaseId }),
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      signal,
    },
  );
  requireSuccessfulResponse(response, "shared-note attachment unavailable");
  const contentLength = Number(response.headers.get("content-length"));
  if (
    Number.isFinite(contentLength) &&
    contentLength > MAX_ATTACHMENT_GRANT_RESPONSE_BYTES
  ) {
    throw new NonRetryableSharedNoteError("shared-note attachment unavailable");
  }
  const text = await response.text();
  if (utf8Length(text) > MAX_ATTACHMENT_GRANT_RESPONSE_BYTES) {
    throw new NonRetryableSharedNoteError("shared-note attachment unavailable");
  }
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new NonRetryableSharedNoteError("shared-note attachment unavailable");
  }
  let download: SharedAttachmentDownload | undefined;
  try {
    download = parsePreviewAttachments([attachment], [value])
      .attachmentDownloads[0];
  } catch {
    throw new NonRetryableSharedNoteError("shared-note attachment unavailable");
  }
  if (!download) {
    throw new NonRetryableSharedNoteError("shared-note attachment unavailable");
  }
  return download;
}

async function withTransientRetries<T>(
  operation: () => Promise<T>,
  signal: AbortSignal,
): Promise<T> {
  for (let attempt = 0; attempt < TRANSIENT_RETRY_ATTEMPTS; attempt += 1) {
    signal.throwIfAborted();
    try {
      return await operation();
    } catch (error) {
      if (signal.aborted || error instanceof NonRetryableSharedNoteError) {
        throw error;
      }
      if (attempt === TRANSIENT_RETRY_ATTEMPTS - 1) {
        throw error;
      }
      await waitForRetry(
        error instanceof TransientSharedNoteError
          ? error.retryAfterMs
          : TRANSIENT_RETRY_DELAY_MS,
        signal,
      );
    }
  }
  throw new Error("shared-note request unavailable");
}

function requireSuccessfulResponse(response: Response, message: string) {
  if (response.ok) return;
  if (response.status === 429) {
    throw new TransientSharedNoteError(
      message,
      parseRetryAfter(response.headers.get("retry-after")),
    );
  }
  if (
    response.status === 408 ||
    response.status === 425 ||
    response.status >= 500
  ) {
    throw new TransientSharedNoteError(message, TRANSIENT_RETRY_DELAY_MS);
  }
  throw new NonRetryableSharedNoteError(message);
}

function parseRetryAfter(value: string | null) {
  if (!value) return RATE_LIMIT_RETRY_DELAY_MS;
  const seconds = Number(value);
  const delay = Number.isFinite(seconds)
    ? seconds * 1000
    : Date.parse(value) - Date.now();
  if (!Number.isFinite(delay) || delay < 0) return RATE_LIMIT_RETRY_DELAY_MS;
  return Math.min(delay, MAX_RETRY_DELAY_MS);
}

async function waitForRetry(delayMs: number, signal: AbortSignal) {
  signal.throwIfAborted();
  await new Promise<void>((resolve, reject) => {
    const timeoutId = setTimeout(() => {
      signal.removeEventListener("abort", handleAbort);
      resolve();
    }, delayMs);
    const handleAbort = () => {
      clearTimeout(timeoutId);
      reject(signal.reason ?? new DOMException("Aborted", "AbortError"));
    };
    signal.addEventListener("abort", handleAbort, { once: true });
    if (signal.aborted) handleAbort();
  });
}

function parsePreviewAttachments(
  attachmentsValue: unknown,
  downloadsValue: unknown,
): Pick<SharedNotePreviewSnapshot, "attachments" | "attachmentDownloads"> {
  if (
    !Array.isArray(attachmentsValue) ||
    attachmentsValue.length > MAX_PREVIEW_ATTACHMENTS ||
    !Array.isArray(downloadsValue) ||
    (downloadsValue.length !== 0 &&
      downloadsValue.length !== attachmentsValue.length)
  ) {
    throw new Error("invalid shared-note preview snapshot");
  }
  const ids = new Set<string>();
  const attachments = attachmentsValue.map((value) => {
    const attachment = parsePreviewAttachment(value);
    if (ids.has(attachment.id)) {
      throw new Error("invalid shared-note preview snapshot");
    }
    ids.add(attachment.id);
    return attachment;
  });
  const expectedOrigin = requireSupabaseOrigin(downloadsValue.length > 0);
  const downloads = downloadsValue.map((value) => {
    const attachment = parsePreviewAttachment(value);
    if (!isRecord(value) || typeof value.signedUrl !== "string") {
      throw new Error("invalid shared-note preview snapshot");
    }
    const signedUrl = new URL(value.signedUrl);
    if (
      signedUrl.protocol !== "https:" ||
      signedUrl.origin !== expectedOrigin ||
      signedUrl.username ||
      signedUrl.password ||
      signedUrl.hash ||
      typeof value.expiresAt !== "string" ||
      !Number.isFinite(Date.parse(value.expiresAt))
    ) {
      throw new Error("invalid shared-note preview snapshot");
    }
    const expected = attachments.find(
      (candidate) => candidate.id === attachment.id,
    );
    if (!expected || !sameAttachment(expected, attachment)) {
      throw new Error("invalid shared-note preview snapshot");
    }
    return {
      ...attachment,
      signedUrl: value.signedUrl,
      expiresAt: value.expiresAt,
    };
  });
  if (
    new Set(downloads.map((download) => download.id)).size !== downloads.length
  ) {
    throw new Error("invalid shared-note preview snapshot");
  }
  return { attachments, attachmentDownloads: downloads };
}

function parsePreviewAttachment(value: unknown): SharedNoteAttachment {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    !UUID_PATTERN.test(value.id) ||
    typeof value.filename !== "string" ||
    value.filename.length === 0 ||
    value.filename.length > 1024 ||
    value.filename.trim() !== value.filename ||
    typeof value.contentType !== "string" ||
    value.contentType.length === 0 ||
    value.contentType.length > 512 ||
    value.contentType.trim() !== value.contentType ||
    !Number.isSafeInteger(value.sizeBytes) ||
    (value.sizeBytes as number) < 0 ||
    (value.sizeBytes as number) > MAX_PREVIEW_ATTACHMENT_BYTES ||
    typeof value.sha256 !== "string" ||
    !SHA256_PATTERN.test(value.sha256)
  ) {
    throw new Error("invalid shared-note preview snapshot");
  }
  return {
    id: value.id,
    filename: value.filename,
    contentType: value.contentType,
    sizeBytes: value.sizeBytes as number,
    sha256: value.sha256,
  };
}

function requireSupabaseOrigin(required: boolean) {
  if (!required) return "";
  try {
    const url = new URL(env.VITE_SUPABASE_URL ?? "");
    if (url.protocol !== "https:") throw new Error();
    return url.origin;
  } catch {
    throw new Error("invalid shared-note preview snapshot");
  }
}

function sameAttachment(
  left: SharedNoteAttachment,
  right: SharedNoteAttachment,
) {
  return (
    left.id === right.id &&
    left.filename === right.filename &&
    left.contentType === right.contentType &&
    left.sizeBytes === right.sizeBytes &&
    left.sha256 === right.sha256
  );
}

export function SharedNotePreviewAuthLifecycle() {
  const { session } = useAuth();
  useMountEffect(() => {
    void attachmentSyncCommands
      .clearSharedAttachmentPreviewScopes()
      .catch(() => undefined);
  });
  if (session === undefined) {
    return null;
  }
  return <SharedNotePreviewAuthScope key={session?.user.id ?? "signed-out"} />;
}

function SharedNotePreviewAuthScope() {
  useMountEffect(() => purgeAllSharedNotePreviews);
  return null;
}

function clearPreviewAttachmentScope(
  viewId: string,
  cacheTask: Promise<void> | undefined,
) {
  const scopeId = previewAttachmentScopeId(viewId);
  void attachmentTransferNative
    .clearSharedAttachmentScope(scopeId)
    .catch(() => undefined);
  if (cacheTask) {
    const clearAfterCacheSettles = () => {
      void attachmentTransferNative
        .clearSharedAttachmentScope(scopeId)
        .catch(() => undefined);
    };
    void cacheTask.then(clearAfterCacheSettles, clearAfterCacheSettles);
  }
}

function previewAttachmentScopeId(viewId: string) {
  return `${PREVIEW_ATTACHMENT_SCOPE_PREFIX}${viewId}`;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function emitChange() {
  for (const listener of listeners) {
    listener();
  }
}

function utf8Length(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function validatePreviewDocument(root: Record<string, unknown>) {
  let nodes = 0;

  const visit = (value: unknown, depth: number) => {
    if (
      depth > MAX_PREVIEW_DEPTH ||
      !isRecord(value) ||
      typeof value.type !== "string"
    ) {
      throw new Error("invalid shared-note preview snapshot");
    }
    nodes += 1;
    if (nodes > MAX_PREVIEW_NODES) {
      throw new Error("invalid shared-note preview snapshot");
    }

    if (value.content === undefined) {
      return;
    }
    if (!Array.isArray(value.content)) {
      throw new Error("invalid shared-note preview snapshot");
    }
    for (const child of value.content) {
      visit(child, depth + 1);
    }
  };

  visit(root, 0);
}
