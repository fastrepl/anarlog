import assert from "node:assert/strict";
import test from "node:test";

import { getResizedImageSrcSet, getResizedImageUrl } from "./image-cdn.ts";

test("routes local assets through the image cdn", () => {
  assert.equal(
    getResizedImageUrl("/api/assets/team/john.png", { width: 30 }),
    "/.netlify/images?url=%2Fapi%2Fassets%2Fteam%2Fjohn.png&w=30&fm=webp",
  );
});

test("crops when a height is supplied", () => {
  const url = getResizedImageUrl("/api/assets/team/john.png", {
    width: 30,
    height: 30,
  });

  assert.match(url, /h=30/);
  assert.match(url, /fit=cover/);
});

test("leaves remote and already-transformed urls alone", () => {
  const remote = "https://example.com/a.png";
  assert.equal(getResizedImageUrl(remote, { width: 30 }), remote);

  const transformed = "/.netlify/images?url=%2Fa.png&w=30";
  assert.equal(getResizedImageUrl(transformed, { width: 30 }), transformed);
});

test("builds a 1x/2x srcset", () => {
  const srcSet = getResizedImageSrcSet("/api/assets/team/john.png", 30);

  assert.match(srcSet ?? "", /w=30/);
  assert.match(srcSet ?? "", /1x/);
  assert.match(srcSet ?? "", /w=60/);
  assert.match(srcSet ?? "", /2x/);
});

test("skips srcset for remote urls", () => {
  assert.equal(
    getResizedImageSrcSet("https://example.com/a.png", 30),
    undefined,
  );
});
