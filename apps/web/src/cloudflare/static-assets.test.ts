import assert from "node:assert/strict";
import test from "node:test";

import worker from "./static-assets.ts";

test("serves static asset metadata through the shared Supabase handler without leaking app credentials", async (t) => {
  const digest = "a".repeat(64);
  t.mock.method(
    globalThis,
    "fetch",
    async (url: string, options: RequestInit) => {
      assert.equal(
        url,
        `https://ijoptyyjrfqwaqhyxkxj.supabase.co/storage/v1/object/public/public_images/desktop/${digest}/icon.svg`,
      );
      assert.equal(options.method, "HEAD");
      const headers = new Headers(options.headers);
      assert.equal(headers.get("if-none-match"), '"icon"');
      assert.equal(headers.get("cookie"), null);
      assert.equal(headers.get("authorization"), null);
      return new Response(null, {
        headers: { "content-type": "image/svg+xml", etag: '"icon"' },
      });
    },
  );
  const response = await worker.fetch(
    new Request(`https://static.anarlog.so/desktop/${digest}/icon.svg`, {
      method: "HEAD",
      headers: {
        "if-none-match": '"icon"',
        cookie: "session=private",
        authorization: "Bearer private",
      },
    }),
  );
  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get("cache-control"),
    "public, max-age=31536000, immutable",
  );
  assert.equal(response.headers.get("content-type"), "image/svg+xml");
  assert.equal(response.headers.get("etag"), '"icon"');
  assert.equal(
    (
      await worker.fetch(
        new Request("https://other.anarlog.so/desktop/icon.svg"),
      )
    ).status,
    404,
  );
});

test("serves blog preview HEAD requests from the image generator without returning the website HTML response", async (t) => {
  t.mock.method(globalThis, "fetch", async (url: URL, options: RequestInit) => {
    assert.equal(
      url.toString(),
      "https://anarlog.so/api/og/blog/local-ai-privacy-tools",
    );
    assert.equal(options.method, undefined);
    assert.equal(options.headers, undefined);
    return new Response("png", {
      headers: {
        "content-type": "image/png",
        "cache-control": "public, max-age=3600",
        "set-cookie": "private=value",
      },
    });
  });
  const response = await worker.fetch(
    new Request("https://static.anarlog.so/og/blog/local-ai-privacy-tools", {
      method: "HEAD",
      headers: { cookie: "private=value", authorization: "Bearer secret" },
    }),
  );
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-type"), "image/png");
  assert.equal(response.headers.get("set-cookie"), null);
  assert.equal(await response.text(), "");
});
