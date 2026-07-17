import type { Session } from "@supabase/supabase-js";

import type { SharedNoteAttachment } from "./cache";

const MAX_RESPONSE_BYTES = 32 * 1024;
const MAX_SIGNED_URL_BYTES = 16 * 1024;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export type SharedAttachmentDownload = SharedNoteAttachment & {
  signedUrl: string;
  expiresAt: string;
};

export function createSharedAttachmentClient({
  apiBaseUrl,
  session,
  fetcher = fetch,
}: {
  apiBaseUrl: string;
  session: Session;
  fetcher?: typeof fetch;
}) {
  const baseUrl = new URL(apiBaseUrl);
  return {
    async download(
      shareId: string,
      attachmentId: string,
      signal?: AbortSignal,
    ): Promise<SharedAttachmentDownload> {
      requireUuid(shareId);
      requireUuid(attachmentId);
      const response = await fetcher(
        new URL(
          `/shared-notes/access/${shareId}/attachments/${attachmentId}/download`,
          baseUrl,
        ),
        {
          method: "POST",
          headers: {
            Accept: "application/json",
            Authorization: `${session.token_type} ${session.access_token}`,
          },
          cache: "no-store",
          credentials: "omit",
          redirect: "error",
          signal,
        },
      );
      if (!response.ok) {
        throw new SharedAttachmentGatewayError(response.status);
      }
      return parseDownload(await readBoundedJson(response));
    },
  };
}

function parseDownload(value: unknown): SharedAttachmentDownload {
  if (
    !isRecord(value) ||
    Object.keys(value).some(
      (key) =>
        ![
          "id",
          "filename",
          "contentType",
          "sizeBytes",
          "sha256",
          "signedUrl",
          "expiresAt",
        ].includes(key),
    ) ||
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
    (value.sizeBytes as number) > 512 * 1024 * 1024 ||
    typeof value.sha256 !== "string" ||
    !SHA256_PATTERN.test(value.sha256) ||
    typeof value.signedUrl !== "string" ||
    value.signedUrl.length > MAX_SIGNED_URL_BYTES ||
    typeof value.expiresAt !== "string" ||
    !Number.isFinite(Date.parse(value.expiresAt))
  ) {
    throw new Error("invalid shared attachment download grant");
  }
  const signedUrl = new URL(value.signedUrl);
  if (
    signedUrl.protocol !== "https:" ||
    signedUrl.username ||
    signedUrl.password ||
    signedUrl.hash
  ) {
    throw new Error("invalid shared attachment download grant");
  }
  return {
    id: value.id,
    filename: value.filename,
    contentType: value.contentType,
    sizeBytes: value.sizeBytes as number,
    sha256: value.sha256,
    signedUrl: value.signedUrl,
    expiresAt: value.expiresAt,
  };
}

async function readBoundedJson(response: Response) {
  const contentLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(contentLength) && contentLength > MAX_RESPONSE_BYTES) {
    throw new Error("shared attachment download grant is too large");
  }
  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > MAX_RESPONSE_BYTES) {
    throw new Error("shared attachment download grant is too large");
  }
  try {
    return JSON.parse(text) as unknown;
  } catch {
    throw new Error("invalid shared attachment download grant");
  }
}

function requireUuid(value: string) {
  if (!UUID_PATTERN.test(value)) {
    throw new Error("invalid shared attachment ID");
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export class SharedAttachmentGatewayError extends Error {
  constructor(readonly status: number) {
    super(`shared attachment gateway returned ${status}`);
    this.name = "SharedAttachmentGatewayError";
  }
}
