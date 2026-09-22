import assert from "node:assert/strict";
import { test } from "node:test";

import {
  integrationReturnUrl,
  callbackPortSchema,
  returnToDesktop,
} from "./integration-desktop-return.ts";

const callback =
  "anarlog-dev://integration/callback?integration_id=google-drive&status=success&return_to=drive-picker%3Anonce%3Afolder";
test("development callbacks preserve the picker result over loopback", () => {
  assert.equal(
    integrationReturnUrl(callback, 14888),
    "http://127.0.0.1:14888/integration/callback?" + callback.split("?")[1],
  );
  assert.equal(integrationReturnUrl(callback), callback);
  const stable = callback.replace("anarlog-dev:", "anarlog:");
  assert.equal(integrationReturnUrl(stable, 14888), stable);
});
test("callback destinations reject arbitrary ports and routes", () => {
  for (const port of [0, 80, 65536, 1234.5, NaN])
    assert.throws(() => integrationReturnUrl(callback, port));
  assert.throws(() =>
    integrationReturnUrl(
      "anarlog-dev://auth/callback?access_token=secret",
      14888,
    ),
  );
  assert.throws(() => callbackPortSchema.parse("14888@evil.test"));
});
test("loopback delivery does not render the native page or send browser credentials", async (t) => {
  const fetch = t.mock.method(globalThis, "fetch", async () => new Response());
  await returnToDesktop(callback, 14888);
  assert.deepEqual(fetch.mock.calls[0].arguments, [
    integrationReturnUrl(callback, 14888),
    { mode: "no-cors", credentials: "omit", cache: "no-store" },
  ]);
});
