import assert from "node:assert/strict";
import test from "node:test";

import { consumeAuthCallback, pendingAuthStorageKey } from "./auth-callback.ts";
import { buildSignInUrl } from "./sign-in.ts";

test("mobile accepts only the pending login once, including duplicate delivery and expired callbacks", async () => {
  const state = "8d58e770-3a95-41ba-bfa1-ec03633a3a55";
  const values = new Map([
    [pendingAuthStorageKey, JSON.stringify({ state, createdAt: 1_000 })],
  ]);
  const storage = {
    getItem: async (key) => values.get(key) ?? null,
    removeItem: async (key) => {
      values.delete(key);
    },
  };
  const callback = `anarlog://auth/callback?access_token=access&refresh_token=refresh&state=${state}`;
  const consume = (url, now = 1_000) =>
    consumeAuthCallback({ url, scheme: "anarlog", storage, now });
  const signIn = new URL(
    buildSignInUrl("https://anarlog.so", "google", "anarlog", state),
  );
  assert.equal(signIn.searchParams.get("desktop_state"), state);
  assert.equal(await consume(callback.replace(`&state=${state}`, "")), null);
  assert.equal(await consume(callback.replace(state, "forged")), null);
  assert.equal(await consume(callback.replace("anarlog:", "attacker:")), null);
  assert.equal(await consume(callback, 15 * 60 * 1_000 + 1_001), null);
  const results = await Promise.all([consume(callback), consume(callback)]);
  assert.deepEqual(results, [
    { accessToken: "access", refreshToken: "refresh" },
    null,
  ]);
  assert.equal(await consume(callback), null);
});
