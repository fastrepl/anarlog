import { sha256 } from "js-sha256";

const CHUNK_BYTES = 64 * 1024;
const YIELD_AFTER_BYTES = 4 * 1024 * 1024;

function throwIfAborted(signal: AbortSignal | undefined): void {
  if (signal?.aborted) {
    throw signal.reason instanceof Error
      ? signal.reason
      : new Error("Audio verification was cancelled");
  }
}

export async function hashFileSha256(
  file: {
    open: () => {
      close: () => void;
      offset: number | null;
      readBytes: (length: number) => Uint8Array;
      size: number | null;
    };
  },
  signal?: AbortSignal,
): Promise<{ sha256: string; sizeBytes: number }> {
  throwIfAborted(signal);
  const handle = file.open();
  try {
    const expectedSize = handle.size;
    if (expectedSize === null || expectedSize <= 0) {
      throw new Error("Audio file is empty or unavailable");
    }

    handle.offset = 0;
    const digest = sha256.create();
    let sizeBytes = 0;
    let bytesSinceYield = 0;

    while (sizeBytes < expectedSize) {
      throwIfAborted(signal);
      const bytes = handle.readBytes(
        Math.min(CHUNK_BYTES, expectedSize - sizeBytes),
      );
      if (bytes.length === 0) {
        throw new Error("Audio file ended before its reported size");
      }
      digest.update(bytes);
      sizeBytes += bytes.length;
      bytesSinceYield += bytes.length;

      if (bytesSinceYield >= YIELD_AFTER_BYTES) {
        bytesSinceYield = 0;
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
        throwIfAborted(signal);
      }
    }

    if (sizeBytes !== expectedSize) {
      throw new Error("Audio file changed while it was being verified");
    }

    return { sha256: digest.hex(), sizeBytes };
  } finally {
    handle.close();
  }
}
