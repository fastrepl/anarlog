import { describe, expect, test } from "vitest";

import {
  assertAuthorizationState,
  authorizationInputFromParsed,
  chatgptAuthorizeUrl,
  chatgptCodexUrl,
  chatgptResponsesBody,
  CHATGPT_CALLBACK_PORT,
  encodeAuthorizeQuery,
  isSubscriptionProviderId,
  parseAuthorizationInput,
  parseChatgptAccountId,
  parseChatgptResidency,
  subscriptionAuthFromCallback,
  usesSubscriptionFetch,
} from "./oauth";

describe("subscription OAuth helpers", () => {
  test("recognizes subscription provider ids", () => {
    expect(isSubscriptionProviderId("chatgpt")).toBe(true);
    expect(isSubscriptionProviderId("github_copilot")).toBe(true);
    expect(isSubscriptionProviderId("anthropic")).toBe(false);
    expect(isSubscriptionProviderId("claude")).toBe(false);
  });

  test("encodes authorize query spaces as %20 so macOS open does not split the URL", () => {
    expect(
      encodeAuthorizeQuery([["scope", "user:profile user:inference"]]),
    ).toBe("scope=user%3Aprofile%20user%3Ainference");
    expect(
      encodeAuthorizeQuery([["scope", "user:profile user:inference"]]),
    ).not.toContain("+");
  });

  test("builds a ChatGPT authorize URL that returns to the Codex loopback port", () => {
    const href = chatgptAuthorizeUrl({
      challenge: "pkce-challenge",
      state: "oauth-state",
    });
    const url = new URL(href);
    expect(href).not.toContain("+");
    expect(href).toContain(
      `redirect_uri=http%3A%2F%2Flocalhost%3A${CHATGPT_CALLBACK_PORT}%2Fauth%2Fcallback`,
    );
    expect(url.origin + url.pathname).toBe(
      "https://auth.openai.com/oauth/authorize",
    );
    expect(url.searchParams.get("redirect_uri")).toBe(
      `http://localhost:${CHATGPT_CALLBACK_PORT}/auth/callback`,
    );
    expect(url.searchParams.get("originator")).toBe("codex_cli_rs");
    expect(url.searchParams.get("codex_cli_simplified_flow")).toBe("true");
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

  test("turns a callback payload into a code#state value the exchanger already accepts", () => {
    expect(
      authorizationInputFromParsed({ code: "ac_nf5hq", state: "s1" }),
    ).toBe("ac_nf5hq#s1");
    expect(authorizationInputFromParsed({ code: "ac_nf5hq" })).toBe("ac_nf5hq");
  });

  test("ignores Anarlog login callbacks when extracting subscription codes", () => {
    expect(
      subscriptionAuthFromCallback({
        access_token: "access",
        refresh_token: "refresh",
        code: "should-ignore",
      }),
    ).toBeNull();
    expect(
      subscriptionAuthFromCallback({
        code: "codex-code",
        state: "s1",
      }),
    ).toEqual({ code: "codex-code", state: "s1" });
  });

  test("rejects a callback from a different sign-in attempt", () => {
    expect(() =>
      assertAuthorizationState(
        {
          kind: "code",
          url: "https://example.com",
          verifier: "v",
          state: "expected",
        },
        { state: "other" },
      ),
    ).toThrow(/expired/);
    expect(() =>
      assertAuthorizationState(
        {
          kind: "code",
          url: "https://example.com",
          verifier: "v",
          state: "expected",
        },
        { state: "expected" },
      ),
    ).not.toThrow();
  });

  test("uses a subscription fetch wrapper only for OAuth credentials", () => {
    const oauth = JSON.stringify({
      type: "oauth",
      refresh: "r",
      access: "a",
      expires: 1,
    });
    expect(usesSubscriptionFetch("chatgpt", oauth)).toBe(true);
    expect(usesSubscriptionFetch("kimi_code", oauth)).toBe(false);
    expect(usesSubscriptionFetch("chatgpt", "sk-test")).toBe(false);
    expect(usesSubscriptionFetch("anthropic", oauth)).toBe(false);
  });

  test("reads ChatGPT account ids from id-token claims", () => {
    const token = chatgptJwt({
      "https://api.openai.com/auth": { chatgpt_account_id: "acct_workspace" },
    });
    expect(parseChatgptAccountId(token)).toBe("acct_workspace");
    expect(
      parseChatgptAccountId(chatgptJwt({ organizations: [{ id: "org_1" }] })),
    ).toBe("org_1");
    expect(parseChatgptAccountId("sk-not-a-jwt")).toBeUndefined();
  });

  test("reads ChatGPT compute residency from token claims", () => {
    expect(
      parseChatgptResidency(
        chatgptJwt({
          "https://api.openai.com/auth": { chatgpt_compute_residency: "us" },
        }),
      ),
    ).toBe("us");
    expect(
      parseChatgptResidency(
        chatgptJwt({
          "https://api.openai.com/auth": {
            chatgpt_compute_residency: "no_constraint",
          },
        }),
      ),
    ).toBeUndefined();
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

  test("rewrites Codex Responses bodies to the ChatGPT backend contract", () => {
    expect(chatgptResponsesBody('{"model":"gpt-5.4","store":true}')).toBe(
      '{"model":"gpt-5.4","store":false,"stream":true}',
    );
    expect(chatgptResponsesBody('{"model":"gpt-5.4"}')).toBe(
      '{"model":"gpt-5.4","store":false,"stream":true}',
    );
    expect(
      chatgptResponsesBody(
        '{"model":"gpt-5.4","store":false,"max_output_tokens":8192}',
      ),
    ).toBe('{"model":"gpt-5.4","store":false,"stream":true}');
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
