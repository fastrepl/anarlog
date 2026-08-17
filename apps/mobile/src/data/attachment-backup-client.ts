const MAX_RESPONSE_BYTES = 64 * 1024;
const REQUEST_TIMEOUT_MS = 15_000;

export type AttachmentBackupDownload = {
  objectId: string;
  objectKey: string;
  ciphertextSizeBytes: number;
  ciphertextSha256: string;
  formatVersion: number;
  signedUrl: string;
};

export async function requestAttachmentBackupDownload({
  accessToken,
  apiBaseUrl,
  objectKey,
  signal,
}: {
  accessToken: string;
  apiBaseUrl: string;
  objectKey: string;
  signal?: AbortSignal;
}): Promise<AttachmentBackupDownload> {
  if (signal?.aborted) {
    throw signal.reason ?? new Error("Recording download was cancelled.");
  }
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  const abort = () => controller.abort(signal?.reason);
  signal?.addEventListener("abort", abort, { once: true });

  try {
    const response = await fetch(
      new URL(
        "sync/attachment-backups/download",
        ensureTrailingSlash(apiBaseUrl),
      ),
      {
        method: "POST",
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ objectKey }),
        signal: controller.signal,
      },
    );
    const body = await readBoundedBody(response);
    if (!response.ok) {
      throw new Error("The cloud recording is temporarily unavailable.");
    }
    let value: unknown;
    try {
      value = JSON.parse(body);
    } catch {
      throw new Error("The cloud returned an invalid recording download.");
    }
    return parseAttachmentBackupDownload(value, objectKey);
  } catch (error) {
    if (signal?.aborted) {
      throw signal.reason ?? new Error("Recording download was cancelled.");
    }
    if (controller.signal.aborted) {
      throw new Error("The cloud recording request timed out.");
    }
    throw error;
  } finally {
    clearTimeout(timeout);
    signal?.removeEventListener("abort", abort);
  }
}

export function parseAttachmentBackupDownload(
  value: unknown,
  expectedObjectKey: string,
): AttachmentBackupDownload {
  if (!value || typeof value !== "object") {
    throw new Error("The cloud returned an invalid recording download.");
  }
  const result = value as Record<string, unknown>;
  if (
    typeof result.objectId !== "string" ||
    !isUuid(result.objectId) ||
    result.objectKey !== expectedObjectKey ||
    typeof result.ciphertextSizeBytes !== "number" ||
    !Number.isSafeInteger(result.ciphertextSizeBytes) ||
    result.ciphertextSizeBytes <= 0 ||
    typeof result.ciphertextSha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(result.ciphertextSha256) ||
    result.formatVersion !== 1 ||
    typeof result.signedUrl !== "string" ||
    result.signedUrl.length === 0 ||
    result.signedUrl.length > 8_192
  ) {
    throw new Error("The cloud returned an invalid recording download.");
  }
  return {
    objectId: result.objectId,
    objectKey: expectedObjectKey,
    ciphertextSizeBytes: result.ciphertextSizeBytes,
    ciphertextSha256: result.ciphertextSha256,
    formatVersion: result.formatVersion,
    signedUrl: result.signedUrl,
  };
}

async function readBoundedBody(response: Response): Promise<string> {
  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength > MAX_RESPONSE_BYTES) {
    throw new Error("The cloud returned an invalid recording download.");
  }
  const body = await response.text();
  if (!body || new TextEncoder().encode(body).byteLength > MAX_RESPONSE_BYTES) {
    throw new Error("The cloud returned an invalid recording download.");
  }
  return body;
}

function ensureTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}

function isUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(
    value,
  );
}
