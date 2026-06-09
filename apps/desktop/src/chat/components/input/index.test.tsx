import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { clearContentMock, editorState, shellState } = vi.hoisted(() => ({
  clearContentMock: vi.fn(),
  editorState: {
    json: undefined as unknown,
    onUpdate: undefined as undefined | ((json: unknown) => void),
  },
  shellState: {
    mode: "FloatingOpen" as
      | "FloatingClosed"
      | "FloatingOpen"
      | "RightPanelOpen",
  },
}));

vi.mock("@hypr/editor/chat", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    ChatEditor: React.forwardRef<
      { clearContent: () => void; focus: () => void; getJSON: () => unknown },
      {
        className: string;
        onSubmit: () => void;
        onUpdate: (json: unknown) => void;
      }
    >(function ChatEditor({ className, onUpdate }, ref) {
      editorState.onUpdate = onUpdate;

      React.useImperativeHandle(ref, () => ({
        clearContent: clearContentMock,
        focus: vi.fn(),
        getJSON: () => editorState.json,
      }));

      return <div className={className} data-testid="chat-editor" />;
    }),
  };
});

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: shellState.mode,
    },
  }),
}));

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: true,
    elevatedSurfaceClassName: "bg-accent text-accent-foreground border-border",
    inputEditorClassName: "chat-input-editor text-accent-foreground",
    sendButtonDisabledClassName:
      "cursor-default border-border text-muted-foreground/60",
    sendButtonShortcutDisabledClassName: "text-muted-foreground/60",
  }),
}));

vi.mock("~/editor-bridge/mention-config", () => ({
  useMentionConfig: () => undefined,
}));

import { ChatMessageInput } from "./index";

describe("ChatMessageInput", () => {
  beforeEach(() => {
    cleanup();
    clearContentMock.mockClear();
    editorState.json = { type: "doc", content: [] };
    editorState.onUpdate = undefined;
    shellState.mode = "FloatingOpen";
  });

  it("disables send until the draft has content", () => {
    const onSendMessage = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        onSendMessage={onSendMessage}
      />,
    );

    const sendButton = screen.getByRole("button", {
      name: /send/i,
    }) as HTMLButtonElement;
    expect(sendButton.disabled).toBe(true);

    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Hello" }],
        },
      ],
    };
    act(() => {
      editorState.onUpdate?.(editorState.json);
    });

    expect(sendButton.disabled).toBe(false);

    fireEvent.click(sendButton);

    expect(onSendMessage).toHaveBeenCalledWith(
      "Hello",
      [{ type: "text", text: "Hello" }],
      [],
    );
    expect(clearContentMock).toHaveBeenCalled();
  });

  it("marks the send control for disabled surface styling before the draft has content", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });

    expect(sendButton.disabled).toBe(true);
    expect(sendButton.className).toContain("chat-input-send");
    expect(sendButton.className).not.toContain("bg-primary");
  });

  it("uses the elevated dark input surface for typed text", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");
    const surface = editor.closest("[data-chat-message-input]")?.parentElement;

    expect(editor.className).toContain("chat-input-editor");
    expect(surface?.getAttribute("data-chat-input-surface")).toBe("elevated");
    expect(surface?.className).toContain("bg-accent");
    expect(surface?.className).toContain("text-accent-foreground");
  });

  it("uses shared horizontal outer padding while floating", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");
    const outerContainer = messageInput?.parentElement?.parentElement;

    expect(outerContainer?.className).toContain("px-3");
    expect(outerContainer?.className).toContain("pb-2");
    expect(outerContainer?.className).not.toContain("px-2");
    expect(outerContainer?.className).not.toContain("pr-0");
  });

  it("uses balanced outer padding in the right panel", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");
    const outerContainer = messageInput?.parentElement?.parentElement;

    expect(outerContainer?.className).toContain("px-3");
    expect(outerContainer?.className).toContain("pb-4");
    expect(outerContainer?.className).not.toContain("px-5");
    expect(outerContainer?.className).not.toContain("px-2");
    expect(outerContainer?.className).not.toContain("pr-0");
  });
});
