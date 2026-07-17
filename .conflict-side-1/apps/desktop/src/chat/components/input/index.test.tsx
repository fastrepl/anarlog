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
    onSubmit: undefined as undefined | (() => void),
    submitShortcut: undefined as undefined | "mod-enter" | "enter",
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
        placeholder: (props: {
          node: { type: { name: string } };
          pos: number;
        }) => string;
        submitShortcut?: "mod-enter" | "enter";
      }
    >(function ChatEditor(
      { className, onSubmit, onUpdate, placeholder, submitShortcut },
      ref,
    ) {
      editorState.onSubmit = onSubmit;
      editorState.onUpdate = onUpdate;
      editorState.submitShortcut = submitShortcut;

      React.useImperativeHandle(ref, () => ({
        clearContent: clearContentMock,
        focus: vi.fn(),
        getJSON: () => editorState.json,
      }));

      return (
        <div
          className={className}
          data-placeholder={placeholder({
            node: { type: { name: "paragraph" } },
            pos: 0,
          })}
          data-testid="chat-editor"
        />
      );
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
    elevatedSurfaceClassName: "bg-card text-card-foreground border-border",
    inputEditorClassName: "chat-input-editor text-card-foreground",
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
    editorState.onSubmit = undefined;
    editorState.onUpdate = undefined;
    editorState.submitShortcut = undefined;
    shellState.mode = "FloatingOpen";
  });

  it("disables send until the draft has content", () => {
    shellState.mode = "RightPanelOpen";
    const onSendMessage = vi.fn();
    const onDraftContentChange = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        onDraftContentChange={onDraftContentChange}
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
    expect(onDraftContentChange).toHaveBeenCalledWith(true);

    fireEvent.click(sendButton);

    expect(onSendMessage).toHaveBeenCalledWith(
      "Hello",
      [{ type: "text", text: "Hello" }],
      [],
    );
    expect(clearContentMock).toHaveBeenCalled();
    expect(onDraftContentChange).toHaveBeenLastCalledWith(false);
  });

  it("tracks attachment-only drafts without enabling text send", () => {
    shellState.mode = "RightPanelOpen";
    const onDraftContentChange = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        onDraftContentChange={onDraftContentChange}
        onSendMessage={vi.fn()}
      />,
    );

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });

    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "attachment",
              attrs: {
                id: "attachment-1",
                name: "image.png",
                mimeType: "image/png",
                url: "data:image/png;base64,abc",
                size: 123,
              },
            },
          ],
        },
      ],
    };
    act(() => {
      editorState.onUpdate?.(editorState.json);
    });

    expect(onDraftContentChange).toHaveBeenCalledWith(true);
    expect(sendButton.disabled).toBe(true);
  });

  it("submits drafts while streaming so the caller can queue them", () => {
    shellState.mode = "RightPanelOpen";
    const onSendMessage = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        isStreaming
        onSendMessage={onSendMessage}
      />,
    );

    editorState.json = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    };
    act(() => {
      editorState.onUpdate?.(editorState.json);
      editorState.onSubmit?.();
    });

    expect(onSendMessage).toHaveBeenCalledWith(
      "Follow up",
      [{ type: "text", text: "Follow up" }],
      [],
    );
    expect(clearContentMock).toHaveBeenCalled();
  });

  it("configures Enter as the chat submit shortcut", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    expect(editorState.submitShortcut).toBe("enter");
  });

  it("marks the send control for disabled surface styling before the draft has content", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });

    expect(sendButton.disabled).toBe(true);
    expect(sendButton.className).toContain("chat-input-send");
    expect(sendButton.className).not.toContain("bg-primary");
    expect(sendButton.className).toContain("rounded-full");
    expect(sendButton.className).toContain("size-7");
    expect(sendButton.textContent).toBe("");
    expect(sendButton.querySelector("svg")).not.toBeNull();
  });

  it("uses a growable white input surface while floating", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");
    const messageInput = editor.closest("[data-chat-message-input]");
    const surface = messageInput?.parentElement;

    expect(editor.className).toContain("chat-input-editor");
    expect(editor.className).toContain("max-h-36");
    expect(editor.className).toContain("min-h-5");
    expect(editor.className).toContain("overflow-y-auto");
    expect(editor.dataset.placeholder).toBe("Ask anything");
    expect(messageInput?.className).toContain("min-h-[30px]");
    expect(messageInput?.className).toContain("items-center");
    expect(messageInput?.className).not.toContain("items-end");
    expect(messageInput?.className).not.toContain("min-h-10");
    expect(screen.queryByRole("button", { name: /send/i })).toBeNull();
    expect(surface?.getAttribute("data-chat-input-surface")).toBe("floating");
    expect(surface?.className).toContain("min-h-[38px]");
    expect(surface?.className).toContain("max-h-40");
    expect(surface?.className).toContain("items-center");
    expect(surface?.className).not.toContain("items-end");
    expect(surface?.className).toContain("pr-[6px]");
    expect(surface?.className).toContain("pl-4");
    expect(surface?.className).not.toContain("px-4");
    expect(surface?.className).toContain("rounded-[19px]");
    expect(surface?.className).toContain("py-[3px]");
    expect(surface?.className).toContain("bg-white");
    expect(surface?.className).toContain("text-card-foreground");
    expect(surface?.className).toContain("dark:bg-card");
    expect(surface?.className).toContain("dark:text-card-foreground");
    expect(surface?.className).toContain("border-border/70");
    expect(surface?.className).toContain("shadow-none");
    expect(surface?.className).not.toContain("shadow-[");
    expect(surface?.className).not.toContain("inset_0_0_0_1px");
    expect(surface?.className).not.toContain("h-10");
    expect(surface?.className).not.toContain("max-h-10");
    expect(surface?.className).not.toContain("max-h-28");
  });

  it("anchors the floating send control to the bottom edge", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

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

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });
    const sendControl = sendButton.parentElement;
    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");

    expect(sendControl?.className).toContain("absolute");
    expect(sendControl?.className).toContain("right-0");
    expect(sendControl?.className).toContain("bottom-0.5");
    expect(sendControl?.className).not.toContain("self-end");
    expect(sendControl?.className).not.toContain("ml-3");
    expect(messageInput?.className).toContain("items-center");
    expect(messageInput?.className).toContain("relative");
    expect(messageInput?.className).not.toContain("items-end");
  });

  it("uses the light card input surface in the right panel", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");
    const surface = editor.closest("[data-chat-message-input]")?.parentElement;

    expect(editor.className).toContain("chat-input-editor");
    expect(editor.className).toContain("max-h-[40vh]");
    expect(surface?.getAttribute("data-chat-input-surface")).toBe("elevated");
    expect(surface?.className).toContain("bg-card");
    expect(surface?.className).toContain("text-card-foreground");
    expect(surface?.className).toContain("rounded-xl");
  });

  it("keeps the floating input inset from the clipped shell corners", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");
    const outerContainer = messageInput?.parentElement?.parentElement;

    expect(outerContainer?.className).toContain("px-1");
    expect(outerContainer?.className).toContain("pb-1");
    expect(outerContainer?.className).not.toContain("px-3");
    expect(outerContainer?.className).not.toContain("px-2.5");
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

  it("caps the editor height in the right panel separately", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");

    expect(editor.className).toContain("max-h-[40vh]");
    expect(editor.className).not.toContain("max-h-48");
  });
});
