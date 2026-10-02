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
  },
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
        initialContent?: unknown;
        onSubmit: () => void;
        onUpdate: (json: unknown) => void;
        onHistoryNavigate?: (direction: "prev" | "next") => boolean;
        onAttachmentError?: (message: string) => void;
      }
    >(function ChatEditor(
      {
        initialContent,
        onSubmit,
        onUpdate,
        onHistoryNavigate,
        onAttachmentError,
      },
      ref,
    ) {
      editorState.onSubmit = onSubmit;
      editorState.onUpdate = onUpdate;
      editorState.onHistoryNavigate = onHistoryNavigate;
      editorState.onAttachmentError = onAttachmentError;
      editorState.initialContent = initialContent;

      React.useImperativeHandle(ref, () => ({
        clearContent: clearContentMock,
        focus: () => true,
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

      return <div />;
    }),
  };
});

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    event: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: toastError },
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
    insertTextMock.mockClear();
    clearSentMessages();
    editorState.json = { type: "doc", content: [] };
    editorState.onSubmit = undefined;
    editorState.onUpdate = undefined;
    editorState.onHistoryNavigate = undefined;
    editorState.onAttachmentError = undefined;
    editorState.initialContent = undefined;
    editorState.replacementSelections = [];
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
});
