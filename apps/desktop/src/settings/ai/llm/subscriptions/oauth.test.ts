import { describe, expect, test } from "vitest";

import {
  chatgptCodexUrl,
  chatgptResponsesBody,
  claudeMessagesUrl,
  isSubscriptionProviderId,
  parseAuthorizationInput,
  parseChatgptAccountId,
  usesSubscriptionFetch,
} from "./oauth";

describe("subscription OAuth helpers", () => {
  test("recognizes subscription provider ids", () => {
    expect(isSubscriptionProviderId("claude")).toBe(true);
    expect(isSubscriptionProviderId("github_copilot")).toBe(true);
    expect(isSubscriptionProviderId("anthropic")).toBe(false);
  });

  test("parses Claude code#state values", () => {
    expect(parseAuthorizationInput("abc123#xyz")).toEqual({
      code: "abc123",
      state: "xyz",
    });
  });

  test("parses ChatGPT redirect URLs", () => {
    expect(
      parseAuthorizationInput(
        "http://localhost:1455/auth/callback?code=codex-code&state=s1",
      ),
    ).toEqual({
      code: "codex-code",
      state: "s1",
    });
  });

  test("rejects empty authorization input", () => {
    expect(() => parseAuthorizationInput("  ")).toThrow(/authorization code/);
  });

  test("adds beta=true to Claude message URLs", () => {
    expect(claudeMessagesUrl("https://api.anthropic.com/v1/messages")).toBe(
      "https://api.anthropic.com/v1/messages?beta=true",
    );
    expect(
      claudeMessagesUrl("https://api.anthropic.com/v1/messages?beta=true"),
    ).toBe("https://api.anthropic.com/v1/messages?beta=true");
    expect(claudeMessagesUrl("https://api.anthropic.com/v1/models")).toBe(
      "https://api.anthropic.com/v1/models",
    );
  });

  test("uses a subscription fetch wrapper only for OAuth credentials", () => {
    const oauth = JSON.stringify({
      type: "oauth",
      refresh: "r",
      access: "a",
      expires: 1,
    });
    expect(usesSubscriptionFetch("claude", oauth)).toBe(true);
    expect(usesSubscriptionFetch("kimi_code", oauth)).toBe(false);
    expect(usesSubscriptionFetch("claude", "sk-test")).toBe(false);
    expect(usesSubscriptionFetch("anthropic", oauth)).toBe(false);
  });

  test("reads ChatGPT account ids from id-token claims", () => {
    const token = chatgptJwt({
      "https://api.openai.com/auth": { chatgpt_account_id: "acct_workspace" },
    });
    expect(parseChatgptAccountId(token)).toBe("acct_workspace");
    expect(parseChatgptAccountId("sk-not-a-jwt")).toBeUndefined();
  });

  test("rewrites platform OpenAI URLs onto the Codex backend", () => {
    expect(chatgptCodexUrl("https://api.openai.com/v1/responses")).toBe(
      "https://chatgpt.com/backend-api/codex/responses",
    );
    expect(chatgptCodexUrl("https://api.openai.com/v1/models")).toBe(
      "https://chatgpt.com/backend-api/codex/models",
    );
    expect(
      chatgptCodexUrl("https://chatgpt.com/backend-api/codex/responses"),
    ).toBe("https://chatgpt.com/backend-api/codex/responses");
  });

  test("forces store=false on Codex Responses bodies", () => {
    expect(chatgptResponsesBody('{"model":"gpt-5.4","store":true}')).toBe(
      '{"model":"gpt-5.4","store":false}',
    );
    expect(chatgptResponsesBody('{"model":"gpt-5.4"}')).toBe(
      '{"model":"gpt-5.4","store":false}',
    );
  });
});

function chatgptJwt(payload: Record<string, unknown>): string {
  const json = JSON.stringify(payload);
  const body = btoa(json)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
  return `eyJhbGciOiJub25lIn0.${body}.sig`;
}
