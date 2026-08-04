import assert from "node:assert/strict";
import test from "node:test";

import {
  getDecodedBase64Size,
  MAX_BASE64_LENGTH,
  MAX_MEDIA_BYTES,
  readBoundedBody,
} from "./media-upload-request.ts";

test("rejects a declared request body over the limit without reading it", async () => {
  const request = new Request("https://anarlog.so/api/media-upload", {
    method: "POST",
    headers: { "Content-Length": "9" },
    body: "{}",
  });

  assert.equal(await readBoundedBody(request, 8), null);
  assert.equal(request.bodyUsed, false);
});

test("cancels a chunked request as soon as its body exceeds the limit", async () => {
  const encoder = new TextEncoder();
  let cancelled = false;
  const body = new ReadableStream<Uint8Array>({
    pull(controller) {
      controller.enqueue(encoder.encode("12345"));
    },
    cancel() {
      cancelled = true;
    },
  });
  const request = new Request("https://anarlog.so/api/media-upload", {
    method: "POST",
    body,
    duplex: "half",
  } as RequestInit & { duplex: "half" });

  assert.equal(await readBoundedBody(request, 8), null);
  assert.equal(cancelled, true);
});

test("accepts base64 at the exact decoded media boundary", () => {
  const content = `${"A".repeat(MAX_BASE64_LENGTH - 2)}==`;

  assert.equal(getDecodedBase64Size(content), MAX_MEDIA_BYTES);
});

test("rejects malformed base64", () => {
  for (const content of ["", "A", "AA=A", "AAAA!", "===="]) {
    assert.equal(getDecodedBase64Size(content), null, content);
  }
});
