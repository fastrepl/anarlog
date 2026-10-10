import assert from "node:assert/strict";
import { test } from "node:test";

import {
  completeClaudeConnect,
  pollSubscriptionConnect,
  refreshSubscriptionCredential,
  startSubscriptionConnect,
} from "./subscription-oauth.ts";

test("Claude authorization binds the pasted code to its PKCE session and preserves a refresh token when rotation omits it", async () => {
  const fetcher = async (url, init) => {
    assert.equal(url, "https://platform.claude.com/v1/oauth/token");
    assert.equal(init.redirect, "error");
    const body = JSON.parse(init.body);
    if (body.grant_type === "authorization_code") {
      assert.equal(body.code, "approved-code");
      assert.equal(body.state, "state-1");
      assert.equal(body.code_verifier, "verifier-1");
      return Response.json({
        access_token: "claude-access",
        refresh_token: "claude-refresh",
        expires_in: 3600,
      });
    }
    assert.equal(body.refresh_token, "claude-refresh");
    return Response.json({ access_token: "rotated-access", expires_in: 3600 });
  };
  const session = await startSubscriptionConnect("claude", fetcher, undefined, {
    verifier: "verifier-1",
    challenge: "challenge-1",
    state: "state-1",
  });
  const params = new URL(session.url).searchParams;
  assert.ok(!session.url.includes("+"));
  assert.equal(params.get("scope"), "user:profile user:inference");
  assert.equal(params.get("code_challenge_method"), "S256");
  assert.equal(params.get("code_challenge"), "challenge-1");
  await assert.rejects(
    completeClaudeConnect(session, "approved-code#other-state", fetcher),
    /another sign-in/,
  );
  const credential = await completeClaudeConnect(
    session,
    "approved-code#state-1",
    fetcher,
  );
  const refreshed = await refreshSubscriptionCredential(
    "claude",
    credential,
    fetcher,
  );
  assert.equal(refreshed.access, "rotated-access");
  assert.equal(refreshed.refresh, "claude-refresh");
  await assert.rejects(
    completeClaudeConnect({ ...session, expires: 0 }, "approved-code", fetcher),
    /expired/,
  );
});

test("Copilot and Grok device approval respect slow-down and expiry, and Copilot exchanges the GitHub token before inference", async () => {
  const now = Date.now;
  let clock = now();
  Date.now = () => clock;
  try {
    for (const provider of ["github_copilot", "grok"]) {
      let phase = "slow_down";
      const fetcher = async (url, init) => {
        const body =
          init.headers["Content-Type"] === "application/x-www-form-urlencoded"
            ? Object.fromEntries(new URLSearchParams(init.body))
            : init.body
              ? JSON.parse(init.body)
              : {};
        if (url.endsWith("/device/code"))
          return Response.json({
            user_code: "ABCD-EFGH",
            device_code: "device-1",
            interval: 5,
            expires_in: 900,
            verification_uri:
              provider === "github_copilot"
                ? "https://github.com/login/device"
                : "https://accounts.x.ai/oauth2/device",
          });
        if (url.includes("copilot_internal")) {
          assert.equal(init.headers.Authorization, "Bearer github-access");
          return Response.json({
            token: "copilot-session",
            expires_at: clock / 1000 + 3600,
          });
        }
        assert.equal(body.device_code, "device-1");
        assert.equal(
          body.grant_type,
          "urn:ietf:params:oauth:grant-type:device_code",
        );
        if (phase !== "approved")
          return Response.json(
            { error: phase },
            { status: provider === "grok" ? 400 : 200 },
          );
        return Response.json({
          access_token:
            provider === "github_copilot" ? "github-access" : "grok-access",
          refresh_token: "grok-refresh",
          expires_in: 3600,
        });
      };
      const session = await startSubscriptionConnect(provider, fetcher);
      assert.equal(
        await pollSubscriptionConnect(provider, session, fetcher),
        null,
      );
      clock += 5000;
      assert.equal(
        await pollSubscriptionConnect(provider, session, fetcher),
        null,
      );
      assert.equal(session.interval, 10_000);
      phase = "authorization_pending";
      clock += 10_000;
      assert.equal(
        await pollSubscriptionConnect(provider, session, fetcher),
        null,
      );
      phase = "approved";
      clock += 10_000;
      const credential = await pollSubscriptionConnect(
        provider,
        session,
        fetcher,
      );
      assert.equal(
        credential.access,
        provider === "github_copilot" ? "copilot-session" : "grok-access",
      );
      assert.equal(
        credential.refresh,
        provider === "github_copilot" ? "github-access" : "grok-refresh",
      );
      await assert.rejects(
        pollSubscriptionConnect(provider, { ...session, expires: 0 }, fetcher),
        /expired/,
      );
      const cancelled = new AbortController();
      cancelled.abort();
      await assert.rejects(
        pollSubscriptionConnect(provider, session, fetcher, cancelled.signal),
        { name: "AbortError" },
      );
    }
  } finally {
    Date.now = now;
  }
});
