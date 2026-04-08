import assert from "node:assert/strict";
import test from "node:test";

import { extractBase64Images, normalizeBase64Data } from "./media.ts";

test("extracts inline base64 images with title metadata", () => {
  const markdown =
    '![diagram](data:image/png;base64,QUJDRA== "char-editor-width=80")';

  const images = extractBase64Images(markdown);

  assert.equal(images.length, 1);
  assert.equal(images[0].altText, "diagram");
  assert.equal(images[0].title, "char-editor-width=80");
  assert.equal(images[0].mimeType, "png");
  assert.equal(images[0].base64Data, "QUJDRA==");
});

test("extracts base64 images with extra data-url parameters", () => {
  const markdown = "![photo](data:image/png;name=pasted.png;base64,QUJDRA==)";

  const images = extractBase64Images(markdown);

  assert.equal(images.length, 1);
  assert.equal(images[0].mimeType, "png");
  assert.equal(images[0].base64Data, "QUJDRA==");
});

test("normalizes url-safe and percent-encoded base64 payloads", () => {
  assert.equal(normalizeBase64Data("SGVsbG8"), "SGVsbG8=");
  assert.equal(normalizeBase64Data("YWJjZA%3D%3D"), "YWJjZA==");
  assert.equal(normalizeBase64Data("Pz8_"), "Pz8/");
});
