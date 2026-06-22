import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ChatContent } from "./content";

vi.mock("./body", () => ({
  ChatBody: () => <div data-testid="chat-body" />,
}));

vi.mock("./context-bar", () => ({
  ContextBar: () => <div data-testid="context-bar" />,
}));

vi.mock("./input", () => ({
  ChatMessageInput: () => <div data-testid="chat-input" />,
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
  it("uses shrink-wrapped layout by default for floating chat", () => {
    const container = renderContent();

    expect(container?.className).toContain("shrink-0");
    expect(container?.className).not.toContain("flex-1");
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
});
