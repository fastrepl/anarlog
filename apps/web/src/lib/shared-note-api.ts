import { env } from "@/env";
import { isShareRouteToken } from "@/lib/share-route-privacy";
import {
  handoffRequestIdSchema,
  parseGatewaySharedNote,
  parseSharedNoteAttachmentDownload,
  parseShareHandoff,
  publicShareSlugSchema,
  shareIdSchema,
  type SharedNoteSnapshot,
  type SharedNoteAttachmentDownload,
  type ShareHandoff,
} from "@/lib/shared-notes";

const MAX_RESPONSE_BYTES = 2 * 1024 * 1024 + 16 * 1024;
const MAX_DOWNLOAD_GRANT_BYTES = 32 * 1024;

export async function fetchPublicSharedNote(
  publicSlug: string,
  signal?: AbortSignal,
): Promise<SharedNoteSnapshot | null> {
  const parsedSlug = publicShareSlugSchema.safeParse(publicSlug);
  if (!parsedSlug.success) {
    return null;
  }

  return requestSnapshot(
    `/shared-notes/public/${encodeURIComponent(parsedSlug.data)}`,
    { method: "GET", signal },
  );
}

export async function fetchLinkSharedNote(
  shareId: string,
  token: string,
  signal?: AbortSignal,
): Promise<SharedNoteSnapshot | null> {
  const parsedShareId = shareIdSchema.safeParse(shareId);
  if (!parsedShareId.success || !isShareRouteToken(token)) {
    return null;
  }

  return requestSnapshot(
    `/shared-notes/link/${encodeURIComponent(parsedShareId.data)}`,
    {
      method: "POST",
      signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token }),
    },
  );
}

export async function fetchPublicSharedAttachmentDownload(
  publicSlug: string,
  attachmentId: string,
  signal?: AbortSignal,
): Promise<SharedNoteAttachmentDownload | null> {
  const parsedSlug = publicShareSlugSchema.safeParse(publicSlug);
  const parsedAttachmentId = shareIdSchema.safeParse(attachmentId);
  if (!parsedSlug.success || !parsedAttachmentId.success) return null;
  return requestAttachmentDownload(
    `/shared-notes/public/${encodeURIComponent(parsedSlug.data)}/attachments/${encodeURIComponent(parsedAttachmentId.data)}/download`,
    { method: "POST", signal },
  );
}

export async function fetchLinkSharedAttachmentDownload(
  shareId: string,
  token: string,
  attachmentId: string,
  signal?: AbortSignal,
): Promise<SharedNoteAttachmentDownload | null> {
  const parsedShareId = shareIdSchema.safeParse(shareId);
  const parsedAttachmentId = shareIdSchema.safeParse(attachmentId);
  if (
    !parsedShareId.success ||
    !parsedAttachmentId.success ||
    !isShareRouteToken(token)
  ) {
    return null;
  }
  return requestAttachmentDownload(
    `/shared-notes/link/${encodeURIComponent(parsedShareId.data)}/attachments/${encodeURIComponent(parsedAttachmentId.data)}/download`,
    {
      method: "POST",
      signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token }),
    },
  );
}

export async function createPublicShareHandoff(
  publicSlug: string,
): Promise<ShareHandoff | null> {
  const parsedSlug = publicShareSlugSchema.safeParse(publicSlug);
  if (!parsedSlug.success) {
    return null;
  }

  return requestHandoff(
    `/shared-notes/public/${encodeURIComponent(parsedSlug.data)}/handoff`,
    { method: "POST" },
  );
}

export async function createLinkShareHandoff(
  shareId: string,
  token: string,
): Promise<ShareHandoff | null> {
  const parsedShareId = shareIdSchema.safeParse(shareId);
  if (!parsedShareId.success || !isShareRouteToken(token)) {
    return null;
  }

  return requestHandoff(
    `/shared-notes/link/${encodeURIComponent(parsedShareId.data)}/handoff`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token }),
    },
  );
}

export function buildShareHandoffDeepLink(requestId: string) {
  const parsedRequestId = handoffRequestIdSchema.parse(requestId);
  return `hyprnote://share/open?mode=handoff&request_id=${parsedRequestId}`;
}

async function requestSnapshot(path: string, init: RequestInit) {
  const value = await requestJson(path, init);
  if (value === null) {
    return null;
  }

  try {
    return parseGatewaySharedNote(value);
  } catch {
    return null;
  }
}

async function requestHandoff(path: string, init: RequestInit) {
  const value = await requestJson(path, init);
  if (value === null) {
    return null;
  }

  try {
    return parseShareHandoff(value);
  } catch {
    return null;
  }
}

async function requestAttachmentDownload(path: string, init: RequestInit) {
  const value = await requestJson(path, init, MAX_DOWNLOAD_GRANT_BYTES);
  if (value === null) return null;
  try {
    return parseSharedNoteAttachmentDownload(value);
  } catch {
    return null;
  }
}

async function requestJson(
  path: string,
  init: RequestInit,
  maxResponseBytes = MAX_RESPONSE_BYTES,
) {
  try {
    const response = await fetch(new URL(path, apiBaseUrl()), {
      ...init,
      cache: "no-store",
      credentials: "omit",
      referrerPolicy: "no-referrer",
      headers: {
        Accept: "application/json",
        ...init.headers,
      },
    });
    if (!response.ok) {
      return null;
    }

    const contentLength = Number(response.headers.get("content-length"));
    if (Number.isFinite(contentLength) && contentLength > maxResponseBytes) {
      return null;
    }

    const text = await response.text();
    if (new TextEncoder().encode(text).byteLength > maxResponseBytes) {
      return null;
    }
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function apiBaseUrl() {
  return env.VITE_API_URL.endsWith("/")
    ? env.VITE_API_URL
    : `${env.VITE_API_URL}/`;
}
