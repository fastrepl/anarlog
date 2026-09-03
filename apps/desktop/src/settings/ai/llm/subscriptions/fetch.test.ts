import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  tauriFetch: vi.fn(),
  resolveSubscriptionAccess: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-http", () => ({
  fetch: mocks.tauriFetch,
}));

vi.mock("./access", () => ({
  resolveSubscriptionAccess: mocks.resolveSubscriptionAccess,
}));

import { createSubscriptionFetch } from "./fetch";
import { CLAUDE_CODE_IDENTITY } from "./oauth";

describe("createSubscriptionFetch claude", () => {
  beforeEach(() => {
    mocks.tauriFetch.mockReset();
    mocks.resolveSubscriptionAccess.mockReset();
    mocks.tauriFetch.mockResolvedValue(new Response("ok"));
    mocks.resolveSubscriptionAccess.mockResolvedValue({
      token: "sk-ant-oat01-token",
      credential: {},
    });
  });

  test("swaps the API key for a bearer token and honest OAuth headers", async () => {
    const fetchImpl = createSubscriptionFetch("claude", "stored-credential");

    await fetchImpl("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: {
        "x-api-key": "oauth",
        "anthropic-dangerous-direct-browser-access": "true",
      },
      body: JSON.stringify({
        model: "claude-sonnet-5",
        system: [{ type: "text", text: "Summarize the meeting." }],
      }),
    });

    expect(mocks.tauriFetch).toHaveBeenCalledWith(
      "https://api.anthropic.com/v1/messages",
      expect.objectContaining({
        headers: expect.any(Headers),
        body: JSON.stringify({
          model: "claude-sonnet-5",
          system: [
            { type: "text", text: CLAUDE_CODE_IDENTITY },
            { type: "text", text: "Summarize the meeting." },
          ],
        }),
      }),
    );
    const headers = mocks.tauriFetch.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get("x-api-key")).toBeNull();
    expect(headers.get("Authorization")).toBe("Bearer sk-ant-oat01-token");
    expect(headers.get("anthropic-beta")).toContain("oauth-2025-04-20");
    expect(headers.get("user-agent")).toBeNull();
    expect(headers.get("x-app")).toBeNull();
    // Consumer orgs refuse CORS, so the request must not read as a browser
    // call: no browser-access header, and an empty Origin for the HTTP plugin
    // to strip.
    expect(headers.get("anthropic-dangerous-direct-browser-access")).toBeNull();
    expect(headers.get("Origin")).toBe("");
  });

  test("leaves non-message requests alone", async () => {
    const fetchImpl = createSubscriptionFetch("claude", "stored-credential");

    await fetchImpl("https://api.anthropic.com/v1/models", { method: "GET" });

    expect(mocks.tauriFetch).toHaveBeenCalledWith(
      "https://api.anthropic.com/v1/models",
      expect.objectContaining({ body: undefined }),
    );
  });
});

describe("createSubscriptionFetch chatgpt", () => {
  beforeEach(() => {
    mocks.tauriFetch.mockReset();
    mocks.resolveSubscriptionAccess.mockReset();
    mocks.tauriFetch.mockResolvedValue(new Response("ok"));
    mocks.resolveSubscriptionAccess.mockResolvedValue({
      token: chatgptJwt({
        "https://api.openai.com/auth": {
          chatgpt_account_id: "acct_workspace",
          chatgpt_compute_residency: "us",
        },
      }),
      credential: { accountId: "acct_workspace" },
    });
  });

  test("rewrites Responses calls onto the Codex backend contract", async () => {
    const fetchImpl = createSubscriptionFetch("chatgpt", "stored-credential");

    await fetchImpl("https://api.openai.com/v1/responses", {
      method: "POST",
      body: JSON.stringify({
        model: "gpt-5.4",
        max_output_tokens: 8192,
      }),
    });

    expect(mocks.tauriFetch).toHaveBeenCalledWith(
      "https://chatgpt.com/backend-api/codex/responses",
      expect.objectContaining({
        body: '{"model":"gpt-5.4","store":false,"stream":true}',
        headers: expect.any(Headers),
      }),
    );

    const headers = mocks.tauriFetch.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get("ChatGPT-Account-ID")).toBe("acct_workspace");
    expect(headers.get("originator")).toBe("codex_cli_rs");
    expect(headers.get("session_id")).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i,
    );
    expect(headers.get("x-openai-internal-codex-residency")).toBe("us");
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
