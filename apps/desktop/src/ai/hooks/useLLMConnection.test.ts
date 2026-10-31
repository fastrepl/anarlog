import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { renderHook } from "@testing-library/react";
import { generateText, streamText } from "ai";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  normalizeLLMProviderId,
  useLanguageModel,
  useLLMConnection,
} from "./useLLMConnection";

const mocks = vi.hoisted(() => ({
  provider: "custom" as string,
  model: "mtplx",
  chatProvider: undefined as string | undefined,
  chatModel: undefined as string | undefined,
  chatConnected: true,
}));
const managed = vi.hoisted(() => ({ ready: true, disabled: false }));
vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));
vi.mock("~/settings/queries", () => ({
  useSettingsReady: () => managed.ready,
}));
vi.mock("~/auth", () => ({ useAuth: () => ({ session: null }) }));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: false }),
}));
vi.mock("~/settings/providers", () => ({
  useAiProvider: (_type: string, provider: string) =>
    provider === "openai"
      ? {
          type: "llm",
          base_url: "http://127.0.0.1:9000/v1",
          api_key: mocks.chatConnected ? "chat-key" : "",
        }
      : {
          type: "llm",
          base_url: "http://127.0.0.1:8000/v1",
          api_key: "local-key",
        },
}));
vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({
    intelligence_disabled: managed.disabled,
    current_llm_provider: mocks.provider,
    current_llm_model: mocks.model,
    current_llm_reasoning_effort: "high",
    chat_llm_provider: mocks.chatProvider,
    chat_llm_model: mocks.chatModel,
  }),
}));

afterEach(() => {
  mocks.provider = "custom";
  mocks.model = "mtplx";
  mocks.chatProvider = undefined;
  mocks.chatModel = undefined;
  mocks.chatConnected = true;
});

it("follows Intelligence until chat is selected, then keeps chat separate from summaries without a silent fallback", () => {
  const useModels = () => ({
    chat: useLLMConnection("chat"),
    chatModel: useLanguageModel("chat"),
    summary: useLLMConnection("enhance"),
  });
  const { result, rerender, unmount } = renderHook(useModels);
  expect(result.current.chat.conn?.modelId).toBe("mtplx");
  mocks.model = "summary-v2";
  rerender();
  expect(result.current.chatModel?.modelId).toBe("summary-v2");

  mocks.chatProvider = "openai";
  mocks.chatModel = "chat-only";
  rerender();
  expect(result.current.chat.conn).toMatchObject({
    providerId: "openai",
    modelId: "chat-only",
    apiKey: "chat-key",
    baseUrl: "http://127.0.0.1:9000/v1",
    reasoningEffort: "default",
  });
  expect(result.current.chatModel?.modelId).toBe("chat-only");
  expect(result.current.summary.conn).toMatchObject({
    providerId: "custom",
    modelId: "summary-v2",
    apiKey: "local-key",
    reasoningEffort: "high",
  });

  mocks.model = "summary-v3";
  rerender();
  expect(result.current.chatModel?.modelId).toBe("chat-only");
  expect(result.current.summary.conn?.modelId).toBe("summary-v3");
  unmount();

  const reopened = renderHook(useModels);
  expect(reopened.result.current.chatModel?.modelId).toBe("chat-only");
  mocks.chatConnected = false;
  reopened.rerender();
  expect(reopened.result.current.chatModel).toBeNull();
  expect(reopened.result.current.chat.status).toMatchObject({
    status: "error",
    reason: "missing_config",
    providerId: "openai",
  });
  expect(reopened.result.current.summary.conn?.modelId).toBe("summary-v3");
  mocks.chatProvider = undefined;
  reopened.rerender();
  expect(reopened.result.current.chatModel).toBeNull();
  expect(reopened.result.current.chat.status).toEqual({
    status: "pending",
    reason: "missing_provider",
  });
  reopened.unmount();
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
  it("maps the legacy hosted provider id and preserves current ids", () => {
    expect(normalizeLLMProviderId("hyprnote")).toBe("anarlog");
    expect(normalizeLLMProviderId("openai")).toBe("openai");
  });
});

it.each([
  { ready: true, disabled: true },
  { ready: false, disabled: false },
])(
  "constructs no model before settings load or while intelligence is managed off (%j)",
  (state) => {
    managed.ready = state.ready;
    managed.disabled = state.disabled;
    vi.mocked(tauriFetch).mockClear();
    try {
      const { result, unmount } = renderHook(() => useLanguageModel());
      expect(result.current).toBeNull();
      expect(tauriFetch).not.toHaveBeenCalled();
      unmount();
    } finally {
      managed.ready = true;
      managed.disabled = false;
    }
  },
);
