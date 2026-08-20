import { describe, expect, test } from "vitest";

import {
  claudeMessagesUrl,
  isSubscriptionProviderId,
  parseAuthorizationInput,
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
});
