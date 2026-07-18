import type { Session } from "@supabase/supabase-js";

import type { JSONContent } from "@hypr/editor/note";
import { uploadSharedAttachment } from "@hypr/supabase/storage";

import { attachmentTransferNative } from "~/attachment-sync/native";
import { liveQueryClient, useLiveQuery } from "~/db";
import { flushDatabaseWrites } from "~/db/write-queue";
import type { SharedNoteAttachment } from "~/shared-notes/cache";

const MAX_RESPONSE_BYTES = 64 * 1024;
const REQUEST_TIMEOUT_MS = 15_000;
const FINALIZE_REQUEST_TIMEOUT_MS = 11 * 60 * 1000;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;

type AttachmentRow = {
  id: string;
  filename: string;
  content_type: string;
  size_bytes: number;
  sha256: string;
  source_type: string;
  source_id: string;
  cloud_sync_enabled: number | boolean;
  cloud_object_key: string;
  local_availability: string;
  transfer_direction: string | null;
  transfer_phase: string | null;
  transfer_error: string | null;
};

export type SessionShareAttachment = {
  id: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  sourceType: string;
  sourceId: string;
  cloudSyncEnabled: boolean;
  cloudObjectKey: string;
  localAvailability: "present" | "absent";
  transferDirection: "upload" | "download" | "delete" | null;
  transferPhase: string | null;
  transferError: string;
};

type ReservedSharedAttachment = {
  attachmentId: string;
  objectKey: string;
  objectState: "reserved" | "ready";
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string | null;
};

type SharedAttachmentUploadGrant = {
  attachmentId: string;
  objectKey: string;
  objectState: "reserved" | "ready";
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  uploadToken: string | null;
};

const SESSION_ATTACHMENTS_SQL = `
  SELECT
    attachment.id,
    attachment.filename,
    attachment.content_type,
    attachment.size_bytes,
    attachment.sha256,
    attachment.source_type,
    attachment.source_id,
    attachment.cloud_sync_enabled,
    attachment.cloud_object_key,
    COALESCE(local.availability, 'absent') AS local_availability,
    job.direction AS transfer_direction,
    job.phase AS transfer_phase,
    job.last_error AS transfer_error
  FROM session_attachments AS attachment
  LEFT JOIN attachment_local_state AS local
    ON local.attachment_id = attachment.id
  LEFT JOIN attachment_transfer_jobs AS job
    ON job.id = (
      SELECT candidate.id
      FROM attachment_transfer_jobs AS candidate
      WHERE candidate.attachment_id = attachment.id
      ORDER BY candidate.updated_at DESC, candidate.created_at DESC, candidate.id
      LIMIT 1
    )
  WHERE attachment.session_id = ?
    AND attachment.deleted_at IS NULL
  ORDER BY attachment.source_type = 'session_audio' DESC,
    attachment.filename COLLATE NOCASE,
    attachment.id
`;

export function useSessionShareAttachments(sessionId: string) {
  return useLiveQuery<AttachmentRow, SessionShareAttachment[]>({
    sql: SESSION_ATTACHMENTS_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: mapAttachmentRows,
  });
}

export async function loadSessionShareAttachments(sessionId: string) {
  await flushDatabaseWrites();
  return mapAttachmentRows(
    await liveQueryClient.execute<AttachmentRow>(SESSION_ATTACHMENTS_SQL, [
      sessionId,
    ]),
  );
}

export async function prepareSessionShareAttachment(input: {
  apiBaseUrl: string;
  supabaseUrl: string;
  session: Session;
  shareId: string;
  attachment: SessionShareAttachment;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
  uploader?: typeof uploadSharedAttachment;
  native?: Pick<
    typeof attachmentTransferNative,
    | "prepareSharedUpload"
    | "readSharedUploadRange"
    | "validateSharedUpload"
    | "cleanupSharedUpload"
  >;
}): Promise<SharedNoteAttachment> {
  const attachment = input.attachment;
  if (
    attachment.localAvailability !== "present" ||
    !attachment.cloudSyncEnabled ||
    !attachment.cloudObjectKey ||
    attachment.sizeBytes <= 0 ||
    !SHA256_PATTERN.test(attachment.sha256)
  ) {
    throw new Error("This attachment is not available on this device.");
  }
  input.signal?.throwIfAborted();
  const contentType = normalizeSharedContentType(attachment.contentType);
  const client = createSharedAttachmentClient(input);
  const [attachmentRef, versionRef] = await Promise.all([
    deriveBlindRef(
      `anarlog-shared-attachment-v1\0${input.shareId}\0${attachment.id}`,
    ),
    deriveBlindRef(
      `anarlog-shared-attachment-version-v1\0${input.shareId}\0${attachment.id}\0${attachment.sha256}\0${attachment.sizeBytes}\0${attachment.filename}\0${contentType}`,
    ),
  ]);
  const reserved = await client.reserve({
    attachmentRef,
    versionRef,
    filename: attachment.filename,
    contentType,
    sizeBytes: attachment.sizeBytes,
  });
  assertSharedAttachmentResponse(reserved, attachment, contentType);
  if (reserved.objectState === "ready") {
    if (reserved.sha256 !== attachment.sha256) {
      throw new Error(
        "The shared attachment version conflicts with the local file.",
      );
    }
    return toSharedNoteAttachment(reserved, attachment);
  }

  const native = input.native ?? attachmentTransferNative;
  const prepared = await native.prepareSharedUpload(
    attachment.id,
    attachment.sha256,
    attachment.sizeBytes,
    attachment.filename,
    attachment.contentType,
    attachment.cloudObjectKey,
    input.signal,
  );
  let operationFailed = false;
  let operationError: unknown;
  try {
    input.signal?.throwIfAborted();
    if (
      prepared.sha256 !== attachment.sha256 ||
      prepared.sizeBytes !== attachment.sizeBytes
    ) {
      throw new Error("The shared attachment upload source was invalid.");
    }
    const grant = await client.grantUpload({
      objectKey: reserved.objectKey,
      sha256: prepared.sha256,
    });
    assertSharedAttachmentResponse(grant, attachment, contentType);
    if (
      grant.attachmentId !== reserved.attachmentId ||
      grant.objectKey !== reserved.objectKey ||
      grant.sha256 !== prepared.sha256
    ) {
      throw new Error("The shared attachment upload grant was invalid.");
    }
    if (grant.uploadToken) {
      const pendingReads = new Set<Promise<Uint8Array>>();
      let uploadStopped = false;
      const readRange = (start: number, end: number) => {
        if (uploadStopped) {
          return Promise.reject(
            new Error("The shared attachment upload stopped."),
          );
        }
        const read = native.readSharedUploadRange(
          attachment.id,
          prepared.cacheId,
          prepared.sha256,
          prepared.sizeBytes,
          attachment.filename,
          attachment.contentType,
          attachment.cloudObjectKey,
          start,
          end,
        );
        pendingReads.add(read);
        void read.then(
          () => pendingReads.delete(read),
          () => pendingReads.delete(read),
        );
        return read;
      };
      const drainReads = async () => {
        while (pendingReads.size > 0) {
          await Promise.allSettled([...pendingReads]);
        }
      };
      const upload = (input.uploader ?? uploadSharedAttachment)({
        objectKey: grant.objectKey,
        signedUploadToken: grant.uploadToken,
        contentType,
        sha256: prepared.sha256,
        sizeBytes: prepared.sizeBytes,
        supabaseUrl: input.supabaseUrl,
        readRange,
      });
      let cancellation: Promise<void> | undefined;
      const cancel = () => (cancellation ??= upload.abort());
      const abort = () => {
        void cancel().catch(() => undefined);
      };
      input.signal?.addEventListener("abort", abort, { once: true });
      if (input.signal?.aborted) abort();
      try {
        await upload.promise;
      } finally {
        uploadStopped = true;
        input.signal?.removeEventListener("abort", abort);
        try {
          if (input.signal?.aborted) await cancel();
        } finally {
          await drainReads();
        }
      }
    }
    input.signal?.throwIfAborted();
    if (
      !(await native.validateSharedUpload(
        attachment.id,
        prepared.cacheId,
        prepared.sha256,
        prepared.sizeBytes,
        attachment.filename,
        attachment.contentType,
        attachment.cloudObjectKey,
        input.signal,
      ))
    ) {
      throw new Error("The local attachment changed during upload.");
    }
    input.signal?.throwIfAborted();
    const finalized = await client.finalize({ objectKey: grant.objectKey });
    if (
      finalized.attachmentId !== reserved.attachmentId ||
      finalized.objectKey !== reserved.objectKey ||
      finalized.objectState !== "ready"
    ) {
      throw new Error("The shared attachment could not be finalized.");
    }
    input.signal?.throwIfAborted();
    if (
      !(await native.validateSharedUpload(
        attachment.id,
        prepared.cacheId,
        prepared.sha256,
        prepared.sizeBytes,
        attachment.filename,
        attachment.contentType,
        attachment.cloudObjectKey,
        input.signal,
      ))
    ) {
      throw new Error("The local attachment changed before publication.");
    }
    input.signal?.throwIfAborted();
    return toSharedNoteAttachment(reserved, attachment);
  } catch (error) {
    operationFailed = true;
    operationError = error;
    throw error;
  } finally {
    try {
      await native.cleanupSharedUpload(prepared.cacheId);
    } catch (error) {
      if (operationFailed) {
        throw Object.assign(
          new Error(
            "The shared attachment upload failed and its snapshot could not be removed.",
          ),
          {
            operationError,
            cleanupError: error,
          },
        );
      }
      throw error;
    }
  }
}

export function matchSharedAttachmentsToLocal(
  localAttachments: SessionShareAttachment[],
  sharedAttachments: SharedNoteAttachment[],
) {
  const remaining = [...localAttachments];
  const localToShared = new Map<string, string>();
  for (const shared of sharedAttachments) {
    const index = remaining.findIndex((local) =>
      attachmentMetadataMatches(local, shared),
    );
    if (index === -1) continue;
    const [local] = remaining.splice(index, 1);
    if (local) localToShared.set(local.id, shared.id);
  }
  return localToShared;
}

export function addSharedAttachmentIds(
  document: JSONContent,
  localAttachments: SessionShareAttachment[],
  localToShared: Map<string, string>,
): JSONContent {
  const sourceToShared = mapLocalSourcesToShared(
    localAttachments,
    localToShared,
  );
  return mapDocumentNode(document, sourceToShared) as JSONContent;
}

export function restoreLocalAttachmentIds(
  document: JSONContent,
  localDocument: JSONContent,
  localAttachments: SessionShareAttachment[],
  localToShared: Map<string, string>,
): JSONContent {
  const sourceToShared = mapLocalSourcesToShared(
    localAttachments,
    localToShared,
  );
  const sharedToSource = new Map<string, string>();
  for (const [sourceId, sharedId] of sourceToShared) {
    sharedToSource.set(sharedId, sourceId);
  }
  const localAttrs = collectLocalAttachmentAttrs(localDocument, sourceToShared);
  return restoreDocumentNode(
    document,
    sharedToSource,
    localAttrs,
  ) as JSONContent;
}

export function isAttachmentShareable(attachment: SessionShareAttachment) {
  return (
    attachment.localAvailability === "present" &&
    attachment.cloudSyncEnabled &&
    attachment.cloudObjectKey.length > 0 &&
    attachment.sizeBytes > 0 &&
    attachment.sizeBytes <= 512 * 1024 * 1024 &&
    SHA256_PATTERN.test(attachment.sha256)
  );
}

function createSharedAttachmentClient(input: {
  apiBaseUrl: string;
  session: Session;
  shareId: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
}) {
  const request = <T>(
    path: string,
    body: unknown,
    timeoutMs = REQUEST_TIMEOUT_MS,
  ) =>
    requestJson<T>({
      fetcher: input.fetcher ?? fetch,
      url: new URL(
        `/sync/shares/${encodeURIComponent(input.shareId)}/attachments/${path}`,
        new URL(input.apiBaseUrl).origin,
      ).toString(),
      accessToken: input.session.access_token,
      body,
      signal: input.signal,
      timeoutMs,
    });
  return {
    reserve: (body: {
      attachmentRef: string;
      versionRef: string;
      filename: string;
      contentType: string;
      sizeBytes: number;
    }) => request<ReservedSharedAttachment>("reserve", body),
    grantUpload: (body: { objectKey: string; sha256: string }) =>
      request<SharedAttachmentUploadGrant>("upload-grant", body),
    finalize: (body: { objectKey: string }) =>
      request<{
        attachmentId: string;
        objectKey: string;
        objectState: "ready";
        wasFinalized: boolean;
      }>("finalize", body, FINALIZE_REQUEST_TIMEOUT_MS),
  };
}

async function requestJson<T>(input: {
  fetcher: typeof fetch;
  url: string;
  accessToken: string;
  body: unknown;
  signal?: AbortSignal;
  timeoutMs: number;
}): Promise<T> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), input.timeoutMs);
  const abort = () => controller.abort();
  input.signal?.addEventListener("abort", abort, { once: true });
  try {
    const response = await input.fetcher(input.url, {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${input.accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(input.body),
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal: controller.signal,
    });
    const text = await readBoundedResponse(response);
    if (!response.ok || !text) {
      throw new Error(`Shared attachment request failed (${response.status}).`);
    }
    return JSON.parse(text) as T;
  } catch (error) {
    if (input.signal?.aborted) {
      throw (
        input.signal.reason ?? new Error("Shared attachment upload aborted.")
      );
    }
    throw error;
  } finally {
    clearTimeout(timeout);
    input.signal?.removeEventListener("abort", abort);
  }
}

async function readBoundedResponse(response: Response) {
  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength > MAX_RESPONSE_BYTES) {
    throw new Error("Shared attachment response was too large.");
  }
  if (!response.body) return response.text();
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Error("Shared attachment response was too large.");
    }
    chunks.push(value);
  }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

function mapAttachmentRows(rows: AttachmentRow[]): SessionShareAttachment[] {
  return rows.map((row) => ({
    id: row.id,
    filename: row.filename,
    contentType: row.content_type,
    sizeBytes: Number(row.size_bytes),
    sha256: row.sha256,
    sourceType: row.source_type,
    sourceId: row.source_id,
    cloudSyncEnabled: Boolean(row.cloud_sync_enabled),
    cloudObjectKey: row.cloud_object_key,
    localAvailability:
      row.local_availability === "present" ? "present" : "absent",
    transferDirection: isTransferDirection(row.transfer_direction)
      ? row.transfer_direction
      : null,
    transferPhase: row.transfer_phase,
    transferError: row.transfer_error ?? "",
  }));
}

function isTransferDirection(
  value: string | null,
): value is "upload" | "download" | "delete" {
  return value === "upload" || value === "download" || value === "delete";
}

function assertSharedAttachmentResponse(
  value: ReservedSharedAttachment | SharedAttachmentUploadGrant,
  attachment: SessionShareAttachment,
  contentType: string,
) {
  if (
    value.filename !== attachment.filename ||
    value.contentType !== contentType ||
    value.sizeBytes !== attachment.sizeBytes ||
    !value.attachmentId ||
    !value.objectKey
  ) {
    throw new Error("The shared attachment response was invalid.");
  }
}

function toSharedNoteAttachment(
  remote: ReservedSharedAttachment,
  local: SessionShareAttachment,
): SharedNoteAttachment {
  return {
    id: remote.attachmentId,
    filename: local.filename,
    contentType: normalizeSharedContentType(local.contentType),
    sizeBytes: local.sizeBytes,
    sha256: local.sha256,
  };
}

function normalizeSharedContentType(value: string) {
  const normalized = value.trim().toLowerCase();
  return normalized || "application/octet-stream";
}

export function attachmentMetadataMatches(
  local: SessionShareAttachment,
  shared: SharedNoteAttachment,
) {
  return (
    local.filename === shared.filename &&
    normalizeSharedContentType(local.contentType) === shared.contentType &&
    local.sizeBytes === shared.sizeBytes &&
    local.sha256 === shared.sha256
  );
}

function mapDocumentNode(
  value: unknown,
  localToShared: Map<string, string>,
): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => mapDocumentNode(item, localToShared));
  }
  if (!value || typeof value !== "object") return value;
  const node = value as Record<string, unknown>;
  const mapped = Object.fromEntries(
    Object.entries(node).map(([key, child]) => [
      key,
      key === "attrs" ? child : mapDocumentNode(child, localToShared),
    ]),
  );
  if (
    (node.type === "image" || node.type === "fileAttachment") &&
    node.attrs &&
    typeof node.attrs === "object"
  ) {
    const attachmentId = (node.attrs as Record<string, unknown>).attachmentId;
    const sharedId =
      typeof attachmentId === "string"
        ? localToShared.get(attachmentId)
        : undefined;
    mapped.attrs = sharedId ? { sharedAttachmentId: sharedId } : {};
  }
  return mapped;
}

function restoreDocumentNode(
  value: unknown,
  sharedToLocal: Map<string, string>,
  localAttrs: Map<string, Record<string, unknown>[]>,
): unknown {
  if (Array.isArray(value)) {
    return value.map((item) =>
      restoreDocumentNode(item, sharedToLocal, localAttrs),
    );
  }
  if (!value || typeof value !== "object") return value;
  const node = value as Record<string, unknown>;
  const mapped = Object.fromEntries(
    Object.entries(node).map(([key, child]) => [
      key,
      key === "attrs"
        ? child
        : restoreDocumentNode(child, sharedToLocal, localAttrs),
    ]),
  );
  if (
    (node.type === "image" ||
      node.type === "fileAttachment" ||
      node.type === "clip") &&
    node.attrs &&
    typeof node.attrs === "object"
  ) {
    const attrs = node.attrs as Record<string, unknown>;
    const sharedId = attrs.sharedAttachmentId;
    if (typeof sharedId === "string") {
      const localId = sharedToLocal.get(sharedId);
      if (!localId) {
        throw new Error("Shared attachment is unavailable locally");
      }
      const preserved = localAttrs
        .get(attachmentNodeKey(node.type, sharedId))
        ?.shift();
      const {
        attachmentId: _,
        sharedAttachmentId: __,
        ...preservedAttrs
      } = preserved ?? {};
      mapped.attrs = { ...preservedAttrs, attachmentId: localId };
    }
  }
  return mapped;
}

function mapLocalSourcesToShared(
  localAttachments: SessionShareAttachment[],
  localToShared: Map<string, string>,
) {
  const sourceToShared = new Map<string, string>();
  for (const attachment of localAttachments) {
    if (attachment.sourceType === "session_audio") continue;
    const sharedId = localToShared.get(attachment.id);
    if (sharedId && attachment.sourceId) {
      sourceToShared.set(attachment.sourceId, sharedId);
    }
  }
  return sourceToShared;
}

function collectLocalAttachmentAttrs(
  value: unknown,
  sourceToShared: Map<string, string>,
  attrs = new Map<string, Record<string, unknown>[]>(),
): Map<string, Record<string, unknown>[]> {
  if (Array.isArray(value)) {
    for (const child of value) {
      collectLocalAttachmentAttrs(child, sourceToShared, attrs);
    }
    return attrs;
  }
  if (!value || typeof value !== "object") return attrs;
  const node = value as Record<string, unknown>;
  if (
    (node.type === "image" || node.type === "fileAttachment") &&
    node.attrs &&
    typeof node.attrs === "object"
  ) {
    const nodeAttrs = node.attrs as Record<string, unknown>;
    const attachmentId = nodeAttrs.attachmentId;
    const sharedId =
      typeof attachmentId === "string"
        ? sourceToShared.get(attachmentId)
        : undefined;
    if (sharedId) {
      const key = attachmentNodeKey(node.type, sharedId);
      const occurrences = attrs.get(key) ?? [];
      occurrences.push(nodeAttrs);
      attrs.set(key, occurrences);
    }
  }
  for (const [key, child] of Object.entries(node)) {
    if (key !== "attrs")
      collectLocalAttachmentAttrs(child, sourceToShared, attrs);
  }
  return attrs;
}

function attachmentNodeKey(type: unknown, sharedId: string) {
  return `${String(type)}\0${sharedId}`;
}

async function deriveBlindRef(value: string) {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(value),
  );
  const base64 = btoa(String.fromCharCode(...new Uint8Array(digest)));
  return base64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
