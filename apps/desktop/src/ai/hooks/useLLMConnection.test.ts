import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { renderHook } from "@testing-library/react";
import { generateText, streamText } from "ai";
import { afterEach, describe, expect, it, vi } from "vitest";

import { normalizeLLMProviderId, useLanguageModel } from "./useLLMConnection";

const mocks = vi.hoisted(() => ({ provider: "custom" as string }));

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));
vi.mock("~/auth", () => ({ useAuth: () => ({ session: null }) }));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: false }),
}));
vi.mock("~/settings/providers", () => ({
  useAiProvider: () => ({
    type: "llm",
    base_url: "http://127.0.0.1:8000/v1",
    api_key: "local-key",
  }),
}));
vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({
    current_llm_provider: mocks.provider,
    current_llm_model: "mtplx",
    current_llm_reasoning_effort: "default",
  }),
}));

afterEach(() => {
  mocks.provider = "custom";
});

it.each([false, true])(
  "generates through Custom with an origin-restricted local server (stream: %s)",
  async (stream) => {
    vi.mocked(tauriFetch).mockImplementation(async (input, init) => {
      const headers = new Headers(init?.headers);
      if (headers.get("Origin") !== "")
        return new Response(null, { status: 403 });
      if (headers.get("Authorization") !== "Bearer local-key")
        return new Response(null, { status: 401 });
      expect(String(input)).toBe("http://127.0.0.1:8000/v1/chat/completions");
      const body = JSON.parse(String(init?.body));
      expect(body.model).toBe("mtplx");
      if (body.stream) {
        const chunk = {
          id: "local-completion",
          model: "mtplx",
          created: 0,
          choices: [
            {
              index: 0,
              delta: { content: "Local summary" },
              finish_reason: "stop",
            },
          ],
        };
        return new Response(
          `data: ${JSON.stringify(chunk)}\n\ndata: [DONE]\n\n`,
          { headers: { "Content-Type": "text/event-stream" } },
        );
      }
      return Response.json({
        id: "local-completion",
        model: "mtplx",
        created: 0,
        choices: [
          {
            index: 0,
            message: { role: "assistant", content: "Local summary" },
            finish_reason: "stop",
          },
        ],
      });
    });

    const { result, unmount } = renderHook(() => useLanguageModel());
    expect(result.current).not.toBeNull();
    const options = {
      model: result.current!,
      prompt: "Summarize the meeting",
      maxRetries: 0,
    };
    const completion = stream
      ? streamText(options)
      : await generateText(options);
    expect(await completion.text).toBe("Local summary");
    unmount();
  },
);

it("sends OpenRouter app-attribution headers for the BYOK openrouter provider", async () => {
  mocks.provider = "openrouter";
  vi.mocked(tauriFetch).mockImplementation(async (input, init) => {
    const headers = new Headers(
      init?.headers ?? (input instanceof Request ? input.headers : undefined),
    );
    expect(headers.get("HTTP-Referer")).toBe("https://anarlog.so");
    expect(headers.get("X-OpenRouter-Title")).toBe("Anarlog");
    expect(headers.get("X-OpenRouter-Categories")).toBe(
      "writing-assistant,personal-agent",
    );
    return Response.json({
      id: "openrouter-completion",
      model: "mtplx",
      created: 0,
      choices: [
        {
          index: 0,
          message: { role: "assistant", content: "Hello from OpenRouter" },
          finish_reason: "stop",
        },
      ],
    });
  });

  const { result, unmount } = renderHook(() => useLanguageModel());
  expect(result.current).not.toBeNull();
  const completion = await generateText({
    model: result.current!,
    prompt: "Summarize the meeting",
    maxRetries: 0,
  });
  expect(await completion.text).toBe("Hello from OpenRouter");
  unmount();
});

describe("normalizeLLMProviderId", () => {
  it("maps the legacy hosted provider id to Anarlog", () => {
    expect(normalizeLLMProviderId("hyprnote")).toBe("anarlog");
  });

  it("preserves current provider ids", () => {
    expect(normalizeLLMProviderId("openai")).toBe("openai");
  });
});
