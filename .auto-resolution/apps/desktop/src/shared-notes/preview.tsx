import { useSyncExternalStore } from "react";

import type { JSONContent } from "@hypr/editor/note";

import { useAuth } from "~/auth";
import { env } from "~/env";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

const MAX_PREVIEW_BODY_BYTES = 2 * 1024 * 1024;
const MAX_PREVIEW_RESPONSE_BYTES = MAX_PREVIEW_BODY_BYTES + 16 * 1024;
const MAX_PREVIEW_TITLE_BYTES = 4096;
const MAX_PREVIEW_DEPTH = 64;
const MAX_PREVIEW_NODES = 50_000;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PREVIEW_KEYS = new Set([
  "shareId",
  "schemaVersion",
  "contentRevision",
  "title",
  "body",
  "publishedAt",
]);

export type SharedNotePreviewSnapshot = {
  shareId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: JSONContent;
  publishedAt: string;
};

export type SharedNotePreviewState =
  | { status: "loading" }
  | { status: "ready"; snapshot: SharedNotePreviewSnapshot }
  | { status: "unavailable" };

type PreviewClaim = (signal: AbortSignal) => Promise<SharedNotePreviewSnapshot>;

const unavailableState = { status: "unavailable" } as const;
const states = new Map<string, SharedNotePreviewState>();
const controllers = new Map<string, AbortController>();
const listeners = new Set<() => void>();

export function beginSharedNotePreview(
  claim: PreviewClaim,
  createViewId: () => string = () => crypto.randomUUID(),
) {
  const viewId = createViewId();
  if (!UUID_PATTERN.test(viewId) || states.has(viewId)) {
    throw new Error("shared-note preview unavailable");
  }

  const controller = new AbortController();
  states.set(viewId, { status: "loading" });
  controllers.set(viewId, controller);
  emitChange();

  void claim(controller.signal)
    .then((snapshot) => {
      if (controllers.get(viewId) !== controller || controller.signal.aborted) {
        return;
      }
      states.set(viewId, { status: "ready", snapshot });
      emitChange();
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
  const deleted = states.delete(viewId);
  if (deleted) {
    emitChange();
  }
}

export function purgeAllSharedNotePreviews() {
  for (const controller of controllers.values()) {
    controller.abort();
  }
  controllers.clear();
  if (states.size > 0) {
    states.clear();
    emitChange();
  }
}

export async function claimSharedNoteHandoff(
  requestId: string,
  signal: AbortSignal,
  fetcher: typeof fetch = fetch,
) {
  if (!UUID_PATTERN.test(requestId)) {
    throw new Error("shared-note handoff unavailable");
  }

  const response = await fetcher(
    new URL("/shared-notes/handoffs/claim", env.VITE_API_URL),
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ requestId }),
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      signal,
    },
  );
  if (!response.ok) {
    throw new Error("shared-note handoff unavailable");
  }

  const contentLength = Number(response.headers.get("content-length"));
  if (
    Number.isFinite(contentLength) &&
    contentLength > MAX_PREVIEW_RESPONSE_BYTES
  ) {
    throw new Error("shared-note handoff unavailable");
  }
  const text = await response.text();
  if (utf8Length(text) > MAX_PREVIEW_RESPONSE_BYTES) {
    throw new Error("shared-note handoff unavailable");
  }

  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("shared-note handoff unavailable");
  }
  return parseSharedNotePreviewSnapshot(value);
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
    publishedAt: value.publishedAt,
  };
}

export function SharedNotePreviewAuthLifecycle() {
  const { session } = useAuth();
  if (session === undefined) {
    return null;
  }
  return <SharedNotePreviewAuthScope key={session?.user.id ?? "signed-out"} />;
}

function SharedNotePreviewAuthScope() {
  useMountEffect(() => purgeAllSharedNotePreviews);
  return null;
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
