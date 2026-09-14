import assert from "node:assert/strict";
import test from "node:test";

import { authSearchSchema, invalidAuthSearchResponse } from "./auth-search.ts";
import { DESKTOP_SCHEMES } from "./desktop-flow.ts";

test("valid web and supported desktop sign-in links pass through", () => {
  const queries = [
    "",
    "flow=web",
    "provider=google",
    "view=sso",
    "redirect=%2Fapp%2F",
    "flow=%22web%22",
  ];
  for (const scheme of DESKTOP_SCHEMES) {
    queries.push(`flow=desktop&scheme=${scheme}&provider=google`);
  }
  for (const query of queries) {
    assert.equal(
      invalidAuthSearchResponse(
        new Request(`https://anarlog.so/auth/?${query}`),
      ),
      undefined,
      query,
    );
  }
  assert.equal(authSearchSchema.parse({}).flow, "web");
});

test("invalid query values return a private 400 without reflecting input", async () => {
  for (const query of [
    "flow=invalid",
    "provider=invalid",
    "view=invalid",
    "scheme=invalid",
    "flow=desktop",
    "flow=web&flow=desktop",
    "flow=null",
    "provider=123",
    "redirect=%7B%22key%22%3A1%7D",
    "flow=" + encodeURIComponent("web' UNION ALL SELECT 'probe',NULL--"),
  ]) {
    for (const path of ["/auth", "/auth/"]) {
      const response = invalidAuthSearchResponse(
        new Request(`https://anarlog.so${path}?${query}`),
      );
      assert.equal(response?.status, 400, query);
      assert.equal(response.headers.get("cache-control"), "no-store");
      assert.equal(
        await response.text(),
        "Invalid sign-in link. Please open /auth/ to sign in.",
      );
    }
  }
});

test("HEAD has no body; unrelated routes and server functions pass through", async () => {
  const response = invalidAuthSearchResponse(
    new Request("https://anarlog.so/auth/?flow=invalid", { method: "HEAD" }),
  );
  assert.equal(response?.status, 400);
  assert.equal(await response.text(), "");
  for (const path of [
    "/callback/auth/",
    "/_server/example",
    "/app/",
    "/api/auth",
  ]) {
    assert.equal(
      invalidAuthSearchResponse(
        new Request(`https://anarlog.so${path}?flow=invalid`),
      ),
      undefined,
    );
  }
  assert.equal(
    invalidAuthSearchResponse(
      new Request("https://anarlog.so/auth/?flow=invalid", { method: "POST" }),
    ),
    undefined,
  );
});
