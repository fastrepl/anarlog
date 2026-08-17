import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { hashFileSha256 } from "./file-sha256.ts";

function memoryFile(bytes, reportedSize = bytes.length) {
  let closed = false;
  let offset = 0;
  return {
    file: {
      open: () => ({
        close: () => {
          closed = true;
        },
        get offset() {
          return offset;
        },
        set offset(nextOffset) {
          offset = nextOffset;
        },
        readBytes: (length) => {
          const chunk = bytes.slice(offset, offset + length);
          offset += chunk.length;
          return chunk;
        },
        size: reportedSize,
      }),
    },
    wasClosed: () => closed,
  };
}

test("hashes a file incrementally and reports the observed size", async () => {
  const bytes = Uint8Array.from({ length: 150_000 }, (_, index) => index % 251);
  const source = memoryFile(bytes);

  assert.deepEqual(await hashFileSha256(source.file), {
    sha256: createHash("sha256").update(bytes).digest("hex"),
    sizeBytes: bytes.length,
  });
  assert.equal(source.wasClosed(), true);
});

test("closes the handle when the file is shorter than its reported size", async () => {
  const source = memoryFile(new Uint8Array([1, 2, 3]), 4);

  await assert.rejects(
    hashFileSha256(source.file),
    /ended before its reported size/,
  );
  assert.equal(source.wasClosed(), true);
});

test("rejects empty files", async () => {
  const source = memoryFile(new Uint8Array());

  await assert.rejects(hashFileSha256(source.file), /empty or unavailable/);
  assert.equal(source.wasClosed(), true);
});

test("does not open a file after verification is cancelled", async () => {
  const controller = new AbortController();
  controller.abort(new Error("cancelled"));
  let opened = false;

  await assert.rejects(
    hashFileSha256(
      {
        open: () => {
          opened = true;
          throw new Error("unexpected open");
        },
      },
      controller.signal,
    ),
    /cancelled/,
  );
  assert.equal(opened, false);
});
