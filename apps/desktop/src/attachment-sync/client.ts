const MAX_RESPONSE_BYTES = 64 * 1024;
const REQUEST_TIMEOUT_MS = 15_000;
const FINALIZE_REQUEST_TIMEOUT_MS = 11 * 60 * 1000;

export type ReservedAttachmentBackup = {
  objectId: string;
  objectKey: string;
  objectState: "reserved" | "ready" | "current";
  ciphertextSizeBytes: number;
  formatVersion: number;
  ciphertextSha256: string | null;
};

export type AttachmentBackupUploadGrant = {
  objectId: string;
  objectKey: string;
  objectState: "reserved" | "ready" | "current";
  ciphertextSizeBytes: number;
  ciphertextSha256: string;
  formatVersion: number;
  uploadExpiresAt: string | null;
  uploadToken: string | null;
};

export type CurrentAttachmentBackup = {
  versionRef: string;
  objectKey: string;
  ciphertextSha256: string;
  ciphertextSizeBytes: number;
  formatVersion: number;
};

export type AttachmentBackupDownload = {
  objectId: string;
  objectKey: string;
  ciphertextSizeBytes: number;
  ciphertextSha256: string;
  formatVersion: number;
  signedUrl: string;
  expiresAt: string;
};

export type AttachmentBackupDeleteRequest = {
  objectKey: string;
  attachmentRef: string;
  versionRef: string;
  deleteRequestId: string;
};

export type ScheduledAttachmentBackupDelete = AttachmentBackupDeleteRequest & {
  deleteFenceId: string;
  deleteGeneration: number;
  deleteNotBefore: string;
};

export function createAttachmentBackupClient(input: {
  apiBaseUrl: string;
  getAccessToken: () => string;
  fetcher?: typeof fetch;
}) {
  const fetcher = input.fetcher ?? fetch;
  const request = <T>(
    path: string,
    init: RequestInit,
    signal?: AbortSignal,
    timeoutMs = REQUEST_TIMEOUT_MS,
  ) =>
    requestJson<T>({
      fetcher,
      url: new URL(path, ensureTrailingSlash(input.apiBaseUrl)).toString(),
      accessToken: input.getAccessToken(),
      init,
      signal,
      timeoutMs,
    });

  return {
    reserve(
      body: {
        attachmentRef: string;
        versionRef: string;
        ciphertextSizeBytes: number;
        formatVersion: number;
      },
      signal?: AbortSignal,
    ) {
      return request<ReservedAttachmentBackup>(
        "sync/attachment-backups/reserve",
        jsonRequest("POST", body),
        signal,
      );
    },
    grantUpload(
      body: { objectKey: string; ciphertextSha256: string },
      signal?: AbortSignal,
    ) {
      return request<AttachmentBackupUploadGrant>(
        "sync/attachment-backups/upload-grant",
        jsonRequest("POST", body),
        signal,
      );
    },
    finalize(objectKey: string, signal?: AbortSignal) {
      return request<{
        objectKey: string;
        objectState: "ready" | "current";
        wasFinalized: boolean;
      }>(
        "sync/attachment-backups/finalize",
        jsonRequest("POST", { objectKey }),
        signal,
        FINALIZE_REQUEST_TIMEOUT_MS,
      );
    },
    promote(
      body: {
        objectKey: string;
        expectedCurrentObjectKey: string | null;
      },
      signal?: AbortSignal,
    ) {
      return request<{
        currentObjectKey: string;
        currentVersionRef: string;
        currentCiphertextSha256: string;
        displacedObjectKey: string | null;
        wasPromoted: boolean;
      }>("sync/attachment-backups/head", jsonRequest("PUT", body), signal);
    },
    async head(
      attachmentRef: string,
      signal?: AbortSignal,
    ): Promise<CurrentAttachmentBackup | null> {
      try {
        return await request<CurrentAttachmentBackup>(
          `sync/attachment-backups/head/${encodeURIComponent(attachmentRef)}`,
          { method: "GET" },
          signal,
        );
      } catch (error) {
        if (
          error instanceof AttachmentBackupGatewayError &&
          error.status === 404
        ) {
          return null;
        }
        throw error;
      }
    },
    download(objectKey: string, signal?: AbortSignal) {
      return request<AttachmentBackupDownload>(
        "sync/attachment-backups/download",
        jsonRequest("POST", { objectKey }),
        signal,
      );
    },
    async scheduleDelete(
      body: AttachmentBackupDeleteRequest,
      signal?: AbortSignal,
    ): Promise<ScheduledAttachmentBackupDelete | null> {
      try {
        return await request<ScheduledAttachmentBackupDelete>(
          "sync/attachment-backups/delete",
          jsonRequest("POST", body),
          signal,
        );
      } catch (error) {
        if (
          error instanceof AttachmentBackupGatewayError &&
          error.status === 404
        ) {
          return null;
        }
        throw error;
      }
    },
    async cancelDelete(
      body: AttachmentBackupDeleteRequest,
      signal?: AbortSignal,
    ) {
      try {
        return await request<AttachmentBackupDeleteRequest>(
          "sync/attachment-backups/delete/cancel",
          jsonRequest("POST", body),
          signal,
        );
      } catch (error) {
        if (
          error instanceof AttachmentBackupGatewayError &&
          error.status === 404
        ) {
          throw new AttachmentBackupGatewayError(
            503,
            "attachment_backup_cancel_unavailable",
          );
        }
        throw error;
      }
    },
  };
}

function jsonRequest(method: "POST" | "PUT", body: unknown): RequestInit {
  return {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  };
}

async function requestJson<T>(input: {
  fetcher: typeof fetch;
  url: string;
  accessToken: string;
  init: RequestInit;
  signal?: AbortSignal;
  timeoutMs: number;
}): Promise<T> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), input.timeoutMs);
  const abort = () => controller.abort();
  input.signal?.addEventListener("abort", abort, { once: true });

  try {
    const headers = new Headers(input.init.headers);
    headers.set("Accept", "application/json");
    headers.set("Authorization", `Bearer ${input.accessToken}`);
    const response = await input.fetcher(input.url, {
      ...input.init,
      headers,
      signal: controller.signal,
    });
    const body = await readBoundedBody(response);
    if (!response.ok) {
      throw new AttachmentBackupGatewayError(
        response.status,
        parseErrorCode(body),
      );
    }
    if (!body) {
      throw new AttachmentBackupGatewayError(502, "empty_response");
    }
    return JSON.parse(body) as T;
  } catch (error) {
    if (input.signal?.aborted) {
      throw input.signal.reason ?? abortError();
    }
    throw error;
  } finally {
    clearTimeout(timeout);
    input.signal?.removeEventListener("abort", abort);
  }
}

async function readBoundedBody(response: Response): Promise<string> {
  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength > MAX_RESPONSE_BYTES) {
    throw new AttachmentBackupGatewayError(502, "response_too_large");
  }
  if (!response.body) return "";

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new AttachmentBackupGatewayError(502, "response_too_large");
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

function parseErrorCode(body: string): string {
  try {
    const value = JSON.parse(body) as {
      code?: unknown;
      error?: { code?: unknown };
    };
    if (typeof value.error?.code === "string") return value.error.code;
    return typeof value.code === "string" ? value.code : "request_failed";
  } catch {
    return "request_failed";
  }
}

export function isAttachmentBackupDependencyAppeared(error: unknown) {
  return (
    error instanceof AttachmentBackupGatewayError &&
    error.status === 409 &&
    error.code === "attachment_backup_dependency_appeared"
  );
}

export function isAttachmentBackupDeleteCancelled(error: unknown) {
  return (
    error instanceof AttachmentBackupGatewayError &&
    error.status === 409 &&
    error.code === "attachment_backup_delete_cancelled"
  );
}

export function isAttachmentBackupDeleteTooLate(error: unknown) {
  return (
    error instanceof AttachmentBackupGatewayError &&
    error.status === 409 &&
    error.code === "attachment_backup_delete_too_late"
  );
}

function ensureTrailingSlash(value: string) {
  return value.endsWith("/") ? value : `${value}/`;
}

function abortError() {
  const error = new Error("Attachment transfer aborted");
  error.name = "AbortError";
  return error;
}

export class AttachmentBackupGatewayError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(`Attachment backup request failed (${status}: ${code})`);
    this.name = "AttachmentBackupGatewayError";
  }
}
