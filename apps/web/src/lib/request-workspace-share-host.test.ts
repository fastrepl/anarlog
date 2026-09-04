import assert from "node:assert/strict";
import test from "node:test";

import { getWorkspaceShareSlugFromHeaders } from "./request-workspace-share-host.ts";

test("reads the workspace slug from the Cloudflare forwarded host", () => {
  const headers = new Headers({
    host: "anarlog.netlify.app",
    "x-forwarded-host": "fastrepl.anarlog.so",
  });

  assert.equal(getWorkspaceShareSlugFromHeaders(headers), "fastrepl");
});

test("does not treat the Netlify origin or reserved hosts as workspaces", () => {
  assert.equal(
    getWorkspaceShareSlugFromHeaders(
      new Headers({ host: "anarlog.netlify.app" }),
    ),
    null,
  );
  assert.equal(
    getWorkspaceShareSlugFromHeaders(
      new Headers({ "x-forwarded-host": "models.anarlog.so" }),
    ),
    null,
  );
});
