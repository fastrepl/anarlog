import assert from "node:assert/strict";
import test from "node:test";

import oauthCallback, {
  config,
} from "../../netlify/edge-functions/oauth-callback.ts";

test("redirects OAuth callbacks to Nango without changing the query string", () => {
  const query =
    "code=a%2Fb+c&state=first&scope=Calendars.Read%20offline_access&state=second&empty=";
  const response = oauthCallback(
    new Request(`https://anarlog.so/oauth/callback?${query}`),
  );

  assert.equal(response.status, 308);
  assert.equal(
    response.headers.get("location"),
    `https://api.nango.dev/oauth/callback?${query}`,
  );
  assert.equal(response.headers.get("cache-control"), "no-store");
});

test("registers only the GET callback route", () => {
  assert.deepEqual(config, {
    path: "/oauth/callback",
    method: ["GET"],
  });
});
