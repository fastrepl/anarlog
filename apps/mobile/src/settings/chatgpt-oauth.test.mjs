import assert from "node:assert/strict";
import { test } from "node:test";

import { parseChatgptSummary } from "../data/chatgpt-summary.ts";
import {
  pollChatgptDeviceCode,
  requestChatgptDeviceCode,
} from "./chatgpt-oauth.ts";

test("mobile ChatGPT sign-in exchanges an approved device code without a localhost callback", async () => {
  const token = `test.${Buffer.from(JSON.stringify({ "https://api.openai.com/auth": { chatgpt_account_id: "workspace-1" } })).toString("base64url")}.signature`;
  let approved = false;
  const fetcher = async (url, init) => {
    assert.equal(init.redirect, "error");
    if (url.endsWith("/usercode")) {
      assert.ok(JSON.parse(init.body).client_id);
      return Response.json({
        device_auth_id: "device-1",
        usercode: "ABCD-EFGH",
        interval: "5",
      });
    }
    if (url.endsWith("/deviceauth/token")) {
      assert.deepEqual(JSON.parse(init.body), {
        device_auth_id: "device-1",
        user_code: "ABCD-EFGH",
      });
      return approved
        ? Response.json({
            authorization_code: "approved-code",
            code_verifier: "verifier",
          })
        : new Response("Awaiting approval", { status: 403 });
    }
    assert.equal(url, "https://auth.openai.com/oauth/token");
    const form = new URLSearchParams(init.body);
    assert.equal(
      form.get("redirect_uri"),
      "https://auth.openai.com/deviceauth/callback",
    );
    assert.equal(form.get("code"), "approved-code");
    assert.equal(form.get("code_verifier"), "verifier");
    return Response.json({
      access_token: token,
      id_token: token,
      refresh_token: "refresh-1",
      expires_in: 3600,
    });
  };
  const session = await requestChatgptDeviceCode(fetcher);
  assert.equal(session.interval, 5000);
  assert.equal(await pollChatgptDeviceCode(session, fetcher), null);
  approved = true;
  const credential = await pollChatgptDeviceCode(session, fetcher);
  assert.equal(credential.accountId, "workspace-1");
  assert.equal(credential.access, token);
  assert.equal(credential.refresh, "refresh-1");
  await assert.rejects(
    pollChatgptDeviceCode({ ...session, expires: 0 }, fetcher),
    /expired/,
  );
  const cancelled = new AbortController();
  cancelled.abort();
  await assert.rejects(
    pollChatgptDeviceCode(session, fetcher, cancelled.signal),
    { name: "AbortError" },
  );
  await assert.rejects(
    requestChatgptDeviceCode(
      async () => new Response('{"access_token":"sensitive-token", broken'),
    ),
    (error) =>
      error.message.includes("invalid response") &&
      !error.message.includes("sensitive-token"),
  );
});

test("ChatGPT summaries exclude reasoning and reject interrupted or failed streams", () => {
  const event = (data) => `data: ${JSON.stringify(data)}\r\n\r\n`;
  const prefix =
    event({
      type: "response.reasoning_summary_text.delta",
      delta: "private reasoning",
    }) + event({ type: "response.output_text.delta", delta: "Partial" });
  const completed = event({
    type: "response.completed",
    response: {
      status: "completed",
      output: [
        {
          type: "reasoning",
          content: [{ type: "output_text", text: "private reasoning" }],
        },
        {
          type: "message",
          content: [{ type: "output_text", text: "## Decisions\nShip it." }],
        },
      ],
    },
  });
  assert.equal(
    parseChatgptSummary(prefix + completed),
    "## Decisions\nShip it.",
  );
  assert.throws(
    () => parseChatgptSummary(prefix + "data: [DONE]\n\n"),
    /interrupted/,
  );
  for (const type of ["response.failed", "response.incomplete", "error"]) {
    assert.throws(
      () => parseChatgptSummary(prefix + event({ type })),
      /could not complete/,
    );
  }
  assert.throws(
    () =>
      parseChatgptSummary(
        event({
          type: "response.completed",
          response: { status: "completed", output: [] },
        }),
      ),
    /empty summary/,
  );
  assert.throws(
    () => parseChatgptSummary('data: {"transcript":"sensitive", broken\n\n'),
    /invalid summary response/,
  );
});
