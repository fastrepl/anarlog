import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { renderHook } from "@testing-library/react";
import { jsonSchema, readUIMessageStream } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useLanguageModel } from "./useLLMConnection";

import {
  buildPersistedChatMessage,
  rowToPersistedChatMessage,
} from "~/chat/store/persisted-messages";
import { CustomChatTransport } from "~/chat/transport";
import type { AnlgUIMessage } from "~/chat/types";

const config = vi.hoisted(() => ({
  intelligence_disabled: false,
  current_llm_provider: "chatgpt",
  current_llm_model: "gpt-5.4",
  current_llm_reasoning_effort: "default",
}));

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));
vi.mock("~/settings/queries", () => ({ useSettingsReady: () => true }));
vi.mock("~/auth", () => ({ useAuth: () => ({ session: null }) }));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: false }),
}));
vi.mock("~/analytics", () => ({ trackAnalyticsEvent: vi.fn() }));
vi.mock("~/settings/providers", () => ({
  useAiProvider: () => ({
    type: "llm",
    base_url: "https://api.openai.com/v1",
    api_key: JSON.stringify({
      type: "oauth",
      access: "test-access",
      refresh: "test-refresh",
      expires: Date.now() + 3_600_000,
    }),
  }),
}));
vi.mock("~/shared/config", () => ({ useConfigValues: () => config }));

beforeEach(() => {
  vi.mocked(tauriFetch).mockReset();
  config.current_llm_reasoning_effort = "default";
});

function responseStream(toolCall = false) {
  const events = [
    {
      type: "response.created",
      response: { id: "resp_test", created_at: 1, model: "gpt-5.4" },
    },
    {
      type: "response.output_item.added",
      output_index: 0,
      item: { type: "reasoning", id: "rs_test" },
    },
    {
      type: "response.output_item.done",
      output_index: 0,
      item: {
        type: "reasoning",
        id: "rs_test",
        encrypted_content: "opaque-test-ciphertext",
      },
    },
    ...(toolCall
      ? [
          {
            type: "response.output_item.added",
            output_index: 1,
            item: {
              type: "function_call",
              id: "fc_test",
              call_id: "call_test",
              name: "lookup",
              arguments: "",
            },
          },
          {
            type: "response.output_item.done",
            output_index: 1,
            item: {
              type: "function_call",
              id: "fc_test",
              call_id: "call_test",
              name: "lookup",
              arguments: "{}",
              status: "completed",
            },
          },
        ]
      : [
          {
            type: "response.output_item.added",
            output_index: 1,
            item: { type: "message", id: "msg_test" },
          },
          {
            type: "response.output_text.delta",
            item_id: "msg_test",
            delta: "Hello",
          },
          {
            type: "response.output_item.done",
            output_index: 1,
            item: { type: "message", id: "msg_test" },
          },
        ]),
    {
      type: "response.completed",
      response: { usage: { input_tokens: 1, output_tokens: 2 } },
    },
  ];
  return new Response(
    events.map((event) => `data: ${JSON.stringify(event)}\n\n`).join(""),
    { headers: { "Content-Type": "text/event-stream" } },
  );
}

function captureRequests(toolCall = false) {
  const requests: {
    store?: boolean;
    stream?: boolean;
    include?: string[];
    max_output_tokens?: number;
    reasoning?: { effort?: string };
    input: Record<string, unknown>[];
  }[] = [];
  vi.mocked(tauriFetch).mockImplementation(async (input, init) => {
    expect(String(input)).toBe(
      "https://chatgpt.com/backend-api/codex/responses",
    );
    requests.push(JSON.parse(String(init?.body)));
    return responseStream(toolCall && requests.length === 1);
  });
  return requests;
}

const userMessage = (text: string): AnlgUIMessage => ({
  id: text,
  role: "user",
  parts: [{ type: "text", text }],
});

async function send(transport: CustomChatTransport, messages: AnlgUIMessage[]) {
  let result: AnlgUIMessage | undefined;
  const stream = await transport.sendMessages({
    chatId: "chat-test",
    trigger: "submit-message",
    messageId: messages[messages.length - 1]?.id,
    messages,
    abortSignal: undefined,
  });
  for await (const message of readUIMessageStream<AnlgUIMessage>({
    stream,
    terminateOnError: true,
  })) {
    result = message;
  }
  expect(result).toBeDefined();
  return result!;
}

function reload(message: AnlgUIMessage) {
  const record = buildPersistedChatMessage({
    message,
    chatGroupId: "chat-test",
    ownerUserId: "user-test",
    status: "ready",
  });
  return rowToPersistedChatMessage({
    id: record.id,
    owner_user_id: record.ownerUserId,
    created_at: record.createdAt,
    chat_group_id: record.chatGroupId,
    role: record.role,
    content: record.content,
    metadata_json: record.metadataJson,
    parts_json: record.partsJson,
    status: record.status,
  }).message;
}

describe("ChatGPT subscription reasoning replay", () => {
  it("replays encrypted reasoning instead of item references across turns and reloads", async () => {
    const requests = captureRequests();
    const { result, unmount } = renderHook(() => useLanguageModel("chat"));
    const transport = new CustomChatTransport(result.current!, {});
    const history = [userMessage("hi")];
    history.push(await send(transport, history));
    const reasoning = history[1].parts.find(
      (part) => part.type === "reasoning",
    );
    expect(reasoning?.providerMetadata?.openai).toEqual({
      itemId: "rs_test",
      reasoningEncryptedContent: "opaque-test-ciphertext",
    });
    await send(transport, [...history, userMessage("continue")]);
    unmount();

    const reopened = renderHook(() => useLanguageModel("chat"));
    await send(new CustomChatTransport(reopened.result.current!, {}), [
      ...history.map(reload),
      userMessage("continue after reload"),
    ]);
    reopened.unmount();

    for (const request of requests) {
      expect(request.store).toBe(false);
      expect(request.stream).toBe(true);
      expect(request.include).toContain("reasoning.encrypted_content");
      expect(request.max_output_tokens).toBeUndefined();
    }
    for (const request of requests.slice(1)) {
      expect(request.input).toContainEqual({
        type: "reasoning",
        id: "rs_test",
        encrypted_content: "opaque-test-ciphertext",
        summary: [],
      });
      expect(request.input).not.toContainEqual(
        expect.objectContaining({ type: "item_reference" }),
      );
    }
  });

  it("omits legacy reasoning without ciphertext while retaining the answer", async () => {
    const requests = captureRequests();
    const { result, unmount } = renderHook(() => useLanguageModel("chat"));
    await send(new CustomChatTransport(result.current!, {}), [
      userMessage("hi"),
      reload({
        id: "legacy",
        role: "assistant",
        parts: [
          {
            type: "reasoning",
            text: "",
            providerMetadata: { openai: { itemId: "rs_old" } },
          },
          {
            type: "text",
            text: "Previous answer",
            providerMetadata: { openai: { itemId: "msg_old" } },
          },
        ],
      }),
      userMessage("continue"),
    ]);
    expect(requests[0].input).not.toContainEqual(
      expect.objectContaining({ type: "item_reference" }),
    );
    expect(requests[0].input).not.toContainEqual(
      expect.objectContaining({ type: "reasoning" }),
    );
    expect(requests[0].input).toContainEqual(
      expect.objectContaining({
        role: "assistant",
        content: [{ type: "output_text", text: "Previous answer" }],
      }),
    );
    unmount();
  });

  it("replays encrypted reasoning between tool-loop steps", async () => {
    const requests = captureRequests(true);
    const execute = vi.fn(async () => "Found it");
    const { result, unmount } = renderHook(() => useLanguageModel("chat"));
    await send(
      new CustomChatTransport(result.current!, {
        lookup: {
          inputSchema: jsonSchema({ type: "object", properties: {} }),
          execute,
        },
      }),
      [userMessage("look it up")],
    );
    expect(execute).toHaveBeenCalledOnce();
    expect(requests).toHaveLength(2);
    expect(requests[1].input).toContainEqual({
      type: "reasoning",
      id: "rs_test",
      encrypted_content: "opaque-test-ciphertext",
      summary: [],
    });
    expect(requests[1].input).toContainEqual(
      expect.objectContaining({
        type: "function_call_output",
        call_id: "call_test",
      }),
    );
    expect(requests[1].input).not.toContainEqual(
      expect.objectContaining({ type: "item_reference" }),
    );
    unmount();
  });
});
