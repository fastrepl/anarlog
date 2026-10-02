import assert from "node:assert/strict";
import test from "node:test";

import { servePublicAsset } from "./public-assets.ts";

test("keeps blog media behind the shared proxy and preserves video ranges without forwarding credentials", async (t) => {
  t.mock.method(
    globalThis,
    "fetch",
    async (url: string, options: RequestInit) => {
      assert.equal(
        url,
        [
          "https://ijoptyyjrfqwaqhyxkxj.supabase.co",
          "storage/v1/object/public/blog",
          "articles/demo/movie%20clip.mp4",
        ].join("/"),
      );
      const headers = new Headers(options.headers);
      assert.equal(headers.get("range"), "bytes=0-3");
      assert.equal(headers.get("cookie"), null);
      assert.equal(headers.get("authorization"), null);
      return new Response("clip", {
        status: 206,
        headers: {
          "content-type": "video/mp4",
          "content-range": "bytes 0-3/20",
          "content-length": "4",
          "cache-control": "max-age=undefined",
          etag: '"movie"',
        },
      });
    },
  );
  const response = await servePublicAsset(
    new Request("https://anarlog.so/api/assets/test", {
      headers: {
        range: "bytes=0-3",
        cookie: "session=private",
        authorization: "Bearer private",
      },
    }),
    "blog/articles/demo/movie%20clip.mp4",
  );
  assert.equal(response.status, 206);
  assert.equal(response.headers.get("content-range"), "bytes 0-3/20");
  assert.equal(response.headers.get("content-type"), "video/mp4");
  assert.equal(response.headers.get("cache-control"), "public, max-age=3600");
  assert.equal(response.headers.get("access-control-allow-origin"), "*");
  assert.equal(await response.text(), "clip");
});

test("rejects asset traversal and handles conditional reads without an upstream body", async (t) => {
  for (const path of [
    "../secret",
    "%2e%2e/secret",
    "a\\b",
    "a//b",
    "%E0%A4%A",
    "blog",
    "https://evil.example/a",
  ]) {
    assert.equal(
      (
        await servePublicAsset(
          new Request("https://anarlog.so/api/assets/a"),
          path,
        )
      ).status,
      404,
    );
  }
  t.mock.method(
    globalThis,
    "fetch",
    async (_url: string, options: RequestInit) => {
      assert.equal(new Headers(options.headers).get("if-none-match"), '"icon"');
      return new Response(null, {
        status: 304,
        headers: {
          etag: '"icon"',
          "cache-control": "public, max-age=31536000",
        },
      });
    },
  );
  const response = await servePublicAsset(
    new Request("https://anarlog.so/api/assets/desktop/icon.svg", {
      headers: { "if-none-match": '"icon"' },
    }),
    "desktop/icon.svg",
  );
  assert.equal(response.status, 304);
  assert.equal(response.headers.get("etag"), '"icon"');
  assert.equal(await response.text(), "");
});
