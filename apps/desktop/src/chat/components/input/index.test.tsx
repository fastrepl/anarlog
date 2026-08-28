import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const {
  clearContentMock,
  dictationState,
  editorState,
  focusMock,
  insertTextMock,
  shellState,
  toastError,
} = vi.hoisted(() => ({
  clearContentMock: vi.fn(),
  dictationState: {
    elapsedSeconds: 0,
    phase: "idle" as "idle" | "starting" | "recording" | "transcribing",
    start: vi.fn(),
    stop: vi.fn(),
  },
  editorState: {
    json: undefined as unknown,
    onUpdate: undefined as undefined | ((json: unknown) => void),
    onSubmit: undefined as undefined | (() => void),
    onHistoryNavigate: undefined as
      | undefined
      | ((direction: "prev" | "next") => boolean),
    onAttachmentError: undefined as undefined | ((message: string) => void),
    initialContent: undefined as unknown,
    replacementSelections: [] as Array<"start" | "end">,
    submitShortcut: undefined as undefined | "mod-enter" | "enter",
  },
  focusMock: vi.fn(() => true),
  insertTextMock: vi.fn(),
  shellState: {
    mode: "FloatingOpen" as
      | "FloatingClosed"
      | "FloatingOpen"
      | "RightPanelOpen",
  },
  toastError: vi.fn(),
}));

vi.mock("@anlg/editor/chat", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    ChatEditor: React.forwardRef<
      {
        clearContent: () => void;
        focus: () => boolean;
        getJSON: () => unknown;
        insertText: (text: string) => void;
        replaceContent: (content: unknown, selection?: "start" | "end") => void;
      },
      {
        className: string;
        initialContent?: unknown;
        onSubmit: () => void;
        onUpdate: (json: unknown) => void;
        onHistoryNavigate?: (direction: "prev" | "next") => boolean;
        onAttachmentError?: (message: string) => void;
        placeholder: (props: {
          node: { type: { name: string } };
          pos: number;
        }) => string;
        submitShortcut?: "mod-enter" | "enter";
      }
    >(function ChatEditor(
      {
        className,
        initialContent,
        onSubmit,
        onUpdate,
        onHistoryNavigate,
        onAttachmentError,
        placeholder,
        submitShortcut,
      },
      ref,
    ) {
      editorState.onSubmit = onSubmit;
      editorState.onUpdate = onUpdate;
      editorState.onHistoryNavigate = onHistoryNavigate;
      editorState.onAttachmentError = onAttachmentError;
      editorState.initialContent = initialContent;
      editorState.submitShortcut = submitShortcut;

      React.useImperativeHandle(ref, () => ({
        clearContent: clearContentMock,
        focus: focusMock,
        getJSON: () => editorState.json,
        insertText: insertTextMock,
        replaceContent: (
          content: unknown,
          selection: "start" | "end" = "end",
        ) => {
          editorState.json = content;
          editorState.replacementSelections.push(selection);
          editorState.onUpdate?.(content);
        },
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

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    event: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: toastError },
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
    elevatedSurfaceStyle: undefined,
  }),
}));

vi.mock("~/editor-bridge/mention-config", () => ({
  useMentionConfig: () => undefined,
}));

vi.mock("./use-dictation", () => ({
  useDictation: () => dictationState,
}));

import { clearSentMessages } from "./history";
import { ChatMessageInput } from "./index";

function docWithText(text: string) {
  return {
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text }] }],
  };
}

describe("ChatMessageInput", () => {
  beforeEach(() => {
    cleanup();
    clearContentMock.mockClear();
    dictationState.elapsedSeconds = 0;
    dictationState.phase = "idle";
    dictationState.start.mockClear();
    dictationState.stop.mockClear();
    focusMock.mockReset();
    focusMock.mockReturnValue(true);
    insertTextMock.mockClear();
    clearSentMessages();
    editorState.json = { type: "doc", content: [] };
    editorState.onSubmit = undefined;
    editorState.onUpdate = undefined;
    editorState.onHistoryNavigate = undefined;
    editorState.onAttachmentError = undefined;
    editorState.initialContent = undefined;
    editorState.replacementSelections = [];
    editorState.submitShortcut = undefined;
    shellState.mode = "FloatingOpen";
    toastError.mockClear();
  });

  it("surfaces attachment rejection messages", () => {
    render(
      <ChatMessageInput
        draftKey="chat-input-attachment-error"
        onSendMessage={vi.fn()}
      />,
    );

    act(() => {
      editorState.onAttachmentError?.("Images must be 8 MB or smaller.");
    });

    expect(toastError).toHaveBeenCalledWith("Images must be 8 MB or smaller.");
  });

  it("disables send until the draft has content", () => {
    shellState.mode = "RightPanelOpen";
    const onSendMessage = vi.fn();
    const onDraftContentChange = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        layout="right-panel"
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
        layout="right-panel"
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
        layout="right-panel"
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

  it("starts voice input from the empty floating composer", () => {
    render(
      <ChatMessageInput draftKey="chat-input-voice" onSendMessage={vi.fn()} />,
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: "Start voice input",
      }),
    );

    expect(dictationState.start).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: /send/i })).toBeNull();
  });

  it("shows recording duration and stops voice input", () => {
    dictationState.phase = "recording";
    dictationState.elapsedSeconds = 3;
    render(
      <ChatMessageInput draftKey="chat-input-voice" onSendMessage={vi.fn()} />,
    );

    expect(screen.getByText("0:03")).not.toBeNull();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Stop voice input",
      }),
    );

    expect(dictationState.stop).toHaveBeenCalledOnce();
  });

  it("keeps the response stop control while voice input is active", () => {
    dictationState.phase = "recording";
    const onStop = vi.fn();
    render(
      <ChatMessageInput
        draftKey="chat-input-voice"
        isStreaming
        onSendMessage={vi.fn()}
        onStop={onStop}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Stop voice input" }),
    ).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Stop response" }));

    expect(onStop).toHaveBeenCalledOnce();
  });

  it("retries focus until the editor view is ready", () => {
    const animationFrames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      animationFrames.push(callback);
      return animationFrames.length;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    focusMock.mockReturnValueOnce(false).mockReturnValue(true);

    const { unmount } = render(
      <ChatMessageInput draftKey="chat-input-focus" onSendMessage={vi.fn()} />,
    );

    expect(focusMock).toHaveBeenCalledOnce();

    act(() => {
      animationFrames.shift()?.(0);
    });

    expect(focusMock).toHaveBeenCalledTimes(2);

    unmount();
    vi.unstubAllGlobals();
  });

  it("disables the send control before the draft has content", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    const sendButton = screen.getByRole<HTMLButtonElement>("button", {
      name: /send/i,
    });

    expect(sendButton.disabled).toBe(true);
    expect(sendButton.textContent).toBe("");
    expect(sendButton.querySelector("svg")).not.toBeNull();
  });

  it("wires the floating editor into its input surface", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const editor = screen.getByTestId("chat-editor");
    const messageInput = editor.closest("[data-chat-message-input]");
    const surface = messageInput?.parentElement;

    expect(editor.className).toContain("chat-input-editor");
    expect(editor.dataset.placeholder).toBe("Ask anything");
    expect((messageInput as HTMLElement | null)?.dataset.chatLayout).toBe(
      "floating",
    );
    expect((messageInput as HTMLElement | null)?.dataset.chatVoiceState).toBe(
      "idle",
    );
    expect(screen.queryByRole("button", { name: /send/i })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Start voice input" }),
    ).not.toBeNull();
    expect(surface?.getAttribute("data-chat-input-surface")).toBe("floating");
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

    expect(sendControl?.hasAttribute("data-chat-input-actions")).toBe(true);
    expect((sendControl as HTMLElement | null)?.dataset.chatLayout).toBe(
      "floating",
    );
    expect(messageInput?.contains(sendControl ?? null)).toBe(true);
  });

  it("wires the right-panel editor into its input surface", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    const editor = screen.getByTestId("chat-editor");
    const surface = editor.closest("[data-chat-message-input]")?.parentElement;

    expect(editor.className).toContain("chat-input-editor");
    expect(
      (editor.closest("[data-chat-message-input]") as HTMLElement | null)
        ?.dataset.chatLayout,
    ).toBe("right-panel");
    expect(surface?.getAttribute("data-chat-input-surface")).toBe("elevated");
  });

  it("marks the floating input container", () => {
    render(
      <ChatMessageInput draftKey="chat-input-test" onSendMessage={vi.fn()} />,
    );

    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");
    const outerContainer = messageInput?.parentElement?.parentElement;

    expect(outerContainer?.hasAttribute("data-chat-input-container")).toBe(
      true,
    );
    expect((outerContainer as HTMLElement | null)?.dataset.chatLayout).toBe(
      "floating",
    );
  });

  it("marks the right-panel input container", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    const messageInput = screen
      .getByTestId("chat-editor")
      .closest("[data-chat-message-input]");
    const outerContainer = messageInput?.parentElement?.parentElement;

    expect(outerContainer?.hasAttribute("data-chat-input-container")).toBe(
      true,
    );
    expect((outerContainer as HTMLElement | null)?.dataset.chatLayout).toBe(
      "right-panel",
    );
  });

  it("walks sent messages with arrow navigation and restores the pending draft", () => {
    shellState.mode = "RightPanelOpen";
    render(
      <ChatMessageInput
        draftKey="chat-input-history-walk"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    for (const text of ["First", "Second"]) {
      editorState.json = docWithText(text);
      act(() => {
        editorState.onUpdate?.(editorState.json);
        editorState.onSubmit?.();
      });
    }

    editorState.json = docWithText("In progress");
    act(() => {
      editorState.onUpdate?.(editorState.json);
    });

    act(() => {
      expect(editorState.onHistoryNavigate?.("prev")).toBe(true);
    });
    expect(editorState.json).toEqual(docWithText("Second"));
    expect(screen.getByText("History 1/2")).not.toBeNull();

    act(() => {
      expect(editorState.onHistoryNavigate?.("prev")).toBe(true);
    });
    expect(editorState.json).toEqual(docWithText("First"));
    expect(screen.getByText("History 2/2")).not.toBeNull();

    act(() => {
      expect(editorState.onHistoryNavigate?.("next")).toBe(true);
    });
    expect(editorState.json).toEqual(docWithText("Second"));

    act(() => {
      expect(editorState.onHistoryNavigate?.("next")).toBe(true);
    });
    expect(editorState.json).toEqual(docWithText("In progress"));
    expect(screen.queryByText(/History/)).toBeNull();
    expect(editorState.replacementSelections).toEqual([
      "start",
      "start",
      "end",
      "end",
    ]);
  });

  it("leaves arrow keys alone without history to walk", () => {
    shellState.mode = "RightPanelOpen";
    render(
      <ChatMessageInput
        draftKey="chat-input-history-empty"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    act(() => {
      expect(editorState.onHistoryNavigate?.("prev")).toBe(false);
      expect(editorState.onHistoryNavigate?.("next")).toBe(false);
    });

    expect(screen.queryByText(/History/)).toBeNull();
  });

  it("drops out of history once the recalled message is edited", () => {
    shellState.mode = "RightPanelOpen";
    render(
      <ChatMessageInput
        draftKey="chat-input-history-edit"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    editorState.json = docWithText("Sent");
    act(() => {
      editorState.onUpdate?.(editorState.json);
      editorState.onSubmit?.();
    });

    act(() => {
      editorState.onHistoryNavigate?.("prev");
    });
    expect(screen.getByText("History 1/1")).not.toBeNull();

    act(() => {
      editorState.onUpdate?.(docWithText("Sent again"));
    });

    expect(screen.queryByText(/History/)).toBeNull();
  });

  it("preserves the pending draft when unmounted while browsing history", () => {
    shellState.mode = "RightPanelOpen";
    const draftKey = "chat-input-history-unmount";
    const { unmount } = render(
      <ChatMessageInput
        draftKey={draftKey}
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    editorState.json = docWithText("Sent");
    act(() => {
      editorState.onUpdate?.(editorState.json);
      editorState.onSubmit?.();
    });

    editorState.json = docWithText("In progress");
    act(() => {
      editorState.onUpdate?.(editorState.json);
      editorState.onHistoryNavigate?.("prev");
    });
    expect(editorState.json).toEqual(docWithText("Sent"));

    unmount();
    render(<ChatMessageInput draftKey={draftKey} onSendMessage={vi.fn()} />);

    expect(editorState.initialContent).toEqual(docWithText("In progress"));
  });

  it("applies StyleX alongside the editor integration hook", () => {
    shellState.mode = "RightPanelOpen";

    render(
      <ChatMessageInput
        draftKey="chat-input-test"
        layout="right-panel"
        onSendMessage={vi.fn()}
      />,
    );

    const editor = screen.getByTestId("chat-editor");

    expect(editor.className).toContain("chat-input-editor");
    expect(editor.className.split(/\s+/u).length).toBeGreaterThan(1);
  });
});
