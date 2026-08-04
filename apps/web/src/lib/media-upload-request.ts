export const MAX_MEDIA_BYTES = 10 * 1024 * 1024;
export const MAX_BASE64_LENGTH = Math.ceil(MAX_MEDIA_BYTES / 3) * 4;
export const MAX_REQUEST_BYTES = MAX_BASE64_LENGTH + 16 * 1024;

export async function readBoundedBody(
  request: Request,
  maxBytes = MAX_REQUEST_BYTES,
) {
  const contentLength = request.headers.get("content-length");
  if (
    contentLength &&
    (!/^\d+$/.test(contentLength) || Number(contentLength) > maxBytes)
  ) {
    return null;
  }

  if (!request.body) return "";

  const reader = request.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let bytesRead = 0;
  let text = "";

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      bytesRead += value.byteLength;
      if (bytesRead > maxBytes) {
        try {
          await reader.cancel();
        } catch {}
        return null;
      }
      text += decoder.decode(value, { stream: true });
    }
    return text + decoder.decode();
  } finally {
    reader.releaseLock();
  }
}

export function getDecodedBase64Size(content: string) {
  if (
    content.length === 0 ||
    content.length % 4 !== 0 ||
    !/^[a-zA-Z0-9+/]+={0,2}$/.test(content)
  ) {
    return null;
  }

  const padding = content.endsWith("==") ? 2 : content.endsWith("=") ? 1 : 0;
  return (content.length / 4) * 3 - padding;
}
