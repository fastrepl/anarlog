import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { clearDraftContent, setDraftContent } from "./input/draft";
import { ChatSession } from "./session-provider";

const {
  useChatMock,
  useTransportMock,
  useChatContextPipelineMock,
  useStoreMock,
  useValuesMock,
} = vi.hoisted(() => ({
  useChatMock: vi.fn(),
  useTransportMock: vi.fn(),
  useChatContextPipelineMock: vi.fn(),
  useStoreMock: vi.fn(),
  useValuesMock: vi.fn(),
}));

vi.mock("@ai-sdk/react", () => ({
  useChat: useChatMock,
}));

vi.mock("~/chat/transport/use-transport", () => ({
  useTransport: useTransportMock,
}));

vi.mock("~/chat/context/use-chat-context-pipeline", () => ({
  useChatContextPipeline: useChatContextPipelineMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "test-store",
  UI: {
    useStore: useStoreMock,
    useValues: useValuesMock,
  },
}));

describe("ChatSession", () => {
  beforeEach(() => {
    useStoreMock.mockReturnValue(null);
    useValuesMock.mockReturnValue({ user_id: "user-1" });
    useTransportMock.mockReturnValue({
      transport: null,
      isSystemPromptReady: true,
    });
    useChatMock.mockReturnValue({
      messages: [],
      sendMessage: vi.fn(),
      regenerate: vi.fn(),
      stop: vi.fn(),
      status: "ready",
      error: undefined,
      setMessages: vi.fn(),
    });
    useChatContextPipelineMock.mockImplementation(
      ({ pendingManualRefs }: { pendingManualRefs: unknown[] }) => ({
        contextEntities: [],
        pendingRefs: pendingManualRefs,
      }),
    );
  });

  afterEach(() => {
    clearDraftContent("draft-session");
    vi.clearAllMocks();
  });

  test("keeps restored draft refs after mount effects run", async () => {
    setDraftContent("draft-session", {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "mention",
              attrs: {
                id: "human-1",
                type: "human",
                label: "John",
              },
            },
          ],
        },
      ],
    });

    const child = vi.fn(() => null);

    render(
      <ChatSession sessionId="draft-session">
        {(props) => child(props)}
      </ChatSession>,
    );

    await act(async () => {});

    expect(child).toHaveBeenCalled();
    expect(child.mock.lastCall?.[0].pendingRefs).toEqual([
      {
        kind: "human",
        key: "human:manual:human-1",
        label: "John",
        source: "draft",
        humanId: "human-1",
      },
    ]);
  });
});
