import assert from "node:assert/strict";
import test from "node:test";

import { mintDesktopSessionForAuthenticatedUser } from "./auth-session.ts";

test("does not mint a desktop session without an authenticated user", async () => {
  let mintedEmail: string | null = null;

  const result = await mintDesktopSessionForAuthenticatedUser({
    getUser: async () => ({
      data: { user: null },
      error: new Error("Not authenticated"),
    }),
    mintSession: async (email) => {
      mintedEmail = email;
      return { access_token: "access-token", refresh_token: "refresh-token" };
    },
  });

  assert.equal(result, null);
  assert.equal(mintedEmail, null);
});

test("mints a desktop session for the authenticated user's email", async () => {
  let mintedEmail: string | null = null;

  const result = await mintDesktopSessionForAuthenticatedUser({
    getUser: async () => ({
      data: { user: { email: "signed-in@example.com" } },
      error: null,
    }),
    mintSession: async (email) => {
      mintedEmail = email;
      return { access_token: "access-token", refresh_token: "refresh-token" };
    },
  });

  assert.equal(mintedEmail, "signed-in@example.com");
  assert.deepEqual(result, {
    access_token: "access-token",
    refresh_token: "refresh-token",
  });
});
