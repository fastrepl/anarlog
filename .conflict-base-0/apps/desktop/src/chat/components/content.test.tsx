import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ChatContent } from "./content";

vi.mock("./body", () => ({
  ChatBody: () => <div data-testid="chat-body" />,
}));

vi.mock("./context-bar", () => ({
  ContextBar: () => <div data-testid="context-bar" />,
}));

vi.mock("./input", () => ({
  ChatMessageInput: ({
    onSendMessage,
  }: {
    onSendMessage: (
      content: string,
      parts: Array<{ type: "text"; text: string }>,
    ) => void;
  }) => (
    <button
      type="button"
      data-testid="chat-input"
      onClick={() =>
        onSendMessage("Queued follow-up", [
          { type: "text", text: "Queued follow-up" },
        ])
      }
    >
      Mock input
    </button>
  ),
}));

class FakeDataTransfer {
  dropEffect = "none";
  private readonly values = new Map<string, string>();

  get types() {
    return Array.from(this.values.keys());
  }

  getData(type: string) {
    return this.values.get(type) ?? "";
  }

  setData(type: string, value: string) {
    this.values.set(type, value);
  }
}

const renderContent = (onAddContextEntity = vi.fn()) => {
  const { container } = render(
    <ChatContent
      sessionId="active-session"
      messages={[]}
      sendMessage={vi.fn()}
      regenerate={vi.fn()}
      stop={vi.fn()}
      status="ready"
      model={{} as never}
      handleSendMessage={vi.fn()}
      contextEntities={[]}
      pendingRefs={[]}
      onAddContextEntity={onAddContextEntity}
      isSystemPromptReady
    />,
  );

  return container.querySelector("[data-chat-content]");
};

describe("ChatContent", () => {
  beforeEach(() => {
    cleanup();
  });

  it("lets floating chat body shrink before the composer is clipped", () => {
    const container = renderContent();

    expect(container?.className).toContain("max-h-full");
    expect(container?.className).not.toContain("flex-1");
    expect(container?.className).not.toContain("shrink-0");
  });

  it("fills available height in the right panel layout", () => {
    const { container } = render(
      <ChatContent
        sessionId="active-session"
        layout="right-panel"
        messages={[]}
        sendMessage={vi.fn()}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="ready"
        model={{} as never}
        handleSendMessage={vi.fn()}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    const content = container.querySelector("[data-chat-content]");

    expect(content?.className).toContain("flex-1");
    expect(content?.className).not.toContain("shrink-0");
  });

  it("adds dropped session refs to chat context", () => {
    const onAddContextEntity = vi.fn();
    const container = renderContent(onAddContextEntity);
    const dataTransfer = new FakeDataTransfer();

    dataTransfer.setData(
      "application/x-anarlog-session-context",
      JSON.stringify({ sessionId: "session-1" }),
    );

    fireEvent.dragOver(container!, { dataTransfer });
    fireEvent.drop(container!, { dataTransfer });

    expect(dataTransfer.dropEffect).toBe("copy");
    expect(onAddContextEntity).toHaveBeenCalledWith({
      kind: "session",
      key: "session:manual:session-1",
      source: "manual",
      sessionId: "session-1",
    });
  });

  it("ignores non-session drops", () => {
    const onAddContextEntity = vi.fn();
    const container = renderContent(onAddContextEntity);
    const dataTransfer = new FakeDataTransfer();

    dataTransfer.setData("text/plain", "Meeting notes");

    fireEvent.drop(container!, { dataTransfer });

    expect(onAddContextEntity).not.toHaveBeenCalled();
  });

  it("queues messages submitted while streaming", () => {
    const handleSendMessage = vi.fn();
    const sendMessage = vi.fn();

    render(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={sendMessage}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="streaming"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    fireEvent.click(screen.getByTestId("chat-input"));

    expect(screen.getByText("Queued follow-up")).toBeTruthy();
    expect(handleSendMessage).not.toHaveBeenCalled();
  });

  it("removes queued messages before they are sent", () => {
    const handleSendMessage = vi.fn();
    const { rerender } = render(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={vi.fn()}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="streaming"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    fireEvent.click(screen.getByTestId("chat-input"));
    fireEvent.click(
      screen.getByRole("button", {
        name: "Remove queued message: Queued follow-up",
      }),
    );

    expect(screen.queryByText("Queued follow-up")).toBeNull();

    rerender(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={vi.fn()}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="ready"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    expect(handleSendMessage).not.toHaveBeenCalled();
  });

  it("sends the next queued message when the chat becomes ready", async () => {
    const handleSendMessage = vi.fn();
    const sendMessage = vi.fn();
    const { rerender } = render(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={sendMessage}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="streaming"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[
          { kind: "session", key: "session:auto", sessionId: "s1" },
        ]}
        isSystemPromptReady
      />,
    );

    fireEvent.click(screen.getByTestId("chat-input"));

    rerender(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={sendMessage}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="ready"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    await waitFor(() => {
      expect(handleSendMessage).toHaveBeenCalledWith(
        "Queued follow-up",
        [{ type: "text", text: "Queued follow-up" }],
        sendMessage,
        [{ kind: "session", key: "session:auto", sessionId: "s1" }],
      );
    });
  });

  it("continues dequeueing if a send does not enter a busy state", async () => {
    const handleSendMessage = vi.fn();
    const sendMessage = vi.fn();
    const { rerender } = render(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={sendMessage}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="streaming"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    fireEvent.click(screen.getByTestId("chat-input"));
    fireEvent.click(screen.getByTestId("chat-input"));

    rerender(
      <ChatContent
        sessionId="active-session"
        messages={[]}
        sendMessage={sendMessage}
        regenerate={vi.fn()}
        stop={vi.fn()}
        status="ready"
        model={{} as never}
        handleSendMessage={handleSendMessage}
        contextEntities={[]}
        pendingRefs={[]}
        isSystemPromptReady
      />,
    );

    await waitFor(() => {
      expect(handleSendMessage).toHaveBeenCalledTimes(2);
    });
    expect(screen.queryByText("Queued follow-up")).toBeNull();
  });
});
