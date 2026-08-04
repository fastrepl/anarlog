export const MAX_TRANSCRIPTION_AUDIO_BYTES = 512 * 1024 * 1024;
export const MAX_TRANSCRIPTION_RESPONSE_BYTES = 16 * 1024 * 1024;
export const MAX_TRANSCRIPTION_WORDS = 150_000;

function responseTooLarge(): Error & { code: string; stage: string } {
  return Object.assign(new Error("STT response is too large"), {
    code: "stt_response_too_large",
    stage: "response",
  });
}

function declaredLengthTooLarge(
  contentLength: string | null,
  maxBytes: number,
): boolean {
  if (contentLength === null) return false;
  const length = Number(contentLength);
  return Number.isFinite(length) && length > maxBytes;
}

function exceedsUtf8Limit(value: string, maxBytes: number): boolean {
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (
      code >= 0xd800 &&
      code <= 0xdbff &&
      index + 1 < value.length &&
      value.charCodeAt(index + 1) >= 0xdc00 &&
      value.charCodeAt(index + 1) <= 0xdfff
    ) {
      bytes += 4;
      index += 1;
    } else {
      bytes += 3;
    }
    if (bytes > maxBytes) return true;
  }
  return false;
}

export function assertBoundedTranscriptionResponse(
  body: string,
  contentLength: string | null,
  maxBytes = MAX_TRANSCRIPTION_RESPONSE_BYTES,
): void {
  if (
    declaredLengthTooLarge(contentLength, maxBytes) ||
    exceedsUtf8Limit(body, maxBytes)
  ) {
    throw responseTooLarge();
  }
}

export function boundedSyntheticTokens(
  transcript: string,
  maxWords: number,
): string[] {
  const tokens: string[] = [];
  for (const match of transcript.matchAll(/\S+/g)) {
    if (tokens.length >= maxWords) {
      throw responseTooLarge();
    }
    tokens.push(match[0]);
  }
  return tokens;
}

export async function readBoundedTranscriptionResponse(
  response: Response,
  maxBytes = MAX_TRANSCRIPTION_RESPONSE_BYTES,
): Promise<string> {
  if (
    declaredLengthTooLarge(response.headers.get("content-length"), maxBytes)
  ) {
    await response.body?.cancel();
    throw responseTooLarge();
  }
  if (!response.body) return "";

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let length = 0;
  let text = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > maxBytes) {
        await reader.cancel();
        throw responseTooLarge();
      }
      text += decoder.decode(value, { stream: true });
    }
    return text + decoder.decode();
  } finally {
    reader.releaseLock();
  }
}
