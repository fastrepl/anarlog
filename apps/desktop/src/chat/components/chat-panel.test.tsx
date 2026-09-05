import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatSession: vi.fn(),
  hasAvailableTranscript: false,
  sessionMode: "inactive",
  requestedLiveTranscription: null as boolean | null,
  liveTranscriptionActive: null as boolean | null,
  toolbarControls: vi.fn((_props: Record<string, unknown>) => (
    <div data-testid="chat-toolbar" />
  )),
  chat: {
    groupId: "group-1",
    scope: "general" as "general" | "automations",
    sessionId: "session-1",
    startNewChat: vi.fn(),
    selectChat: vi.fn(),
  },
  noteFilter: "mine",
  folderFilter: null as string | null,
}));

vi.mock("./toolbar-controls", () => ({
  ChatToolbarControls: (props: Record<string, unknown>) => {
    mocks.toolbarControls(props);
    return (
      <div data-surface={props.surface as string} data-testid="chat-toolbar" />
    );
  },
}));

vi.mock("./body", () => ({
  ChatBody: () => <div data-testid="chat-body" />,
}));

vi.mock("./content", () => ({
  ChatContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("./session-provider", () => ({
  ChatSession: (props: { children: (props: object) => React.ReactNode }) => {
    mocks.chatSession(props);
    return props.children({
      messages: [],
      status: "ready",
      error: undefined,
      regenerate: vi.fn(),
      contextEntities: [],
      sendMessage: vi.fn(),
      pendingRefs: [],
    });
  },
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => ({ id: "model-1" }),
}));

vi.mock("~/chat/store/use-chat-actions", () => ({
  useChatActions: () => ({
    handleSendMessage: vi.fn(),
  }),
}));

vi.mock("./use-session-tab", () => ({
  useSessionTab: () => ({ currentSessionId: "session-1" }),
}));

vi.mock("~/sidebar/note-filter", () => ({
  folderIdForNewNote: (noteFilter: string, folderFilter: string | null) =>
    noteFilter === "mine" && folderFilter !== null ? folderFilter : undefined,
  useSidebarNotes: (
    selector: (state: {
      noteFilter: string;
      folderFilter: string | null;
    }) => unknown,
  ) =>
    selector({
      noteFilter: mocks.noteFilter,
      folderFilter: mocks.folderFilter,
    }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({ chat: mocks.chat }),
}));

vi.mock("~/shared/owner-user", () => ({
  useOwnerUserId: () => "user-1",
}));

vi.mock("~/session/queries", () => ({
  useSessionHasTranscript: (sessionId: string) =>
    Boolean(sessionId) && mocks.hasAvailableTranscript,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      getSessionMode: () => mocks.sessionMode,
      live: {
        requestedLiveTranscription: mocks.requestedLiveTranscription,
        liveTranscriptionActive: mocks.liveTranscriptionActive,
        batchTranscriptionPendingBySession: {},
      },
    }),
}));

import { ChatPanelFrame, ChatSessionHost } from "./chat-panel";

function TestChatPanel({
  layout = "floating",
}: {
  layout?: "floating" | "right-panel";
}) {
  return (
    <ChatSessionHost>
      {(sessionProps) => (
        <ChatPanelFrame layout={layout} sessionProps={sessionProps} />
      )}
    </ChatSessionHost>
  );
}

describe("Chat panel", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatSession.mockClear();
    mocks.chat.scope = "general";
    mocks.noteFilter = "mine";
    mocks.folderFilter = null;
    mocks.hasAvailableTranscript = false;
    mocks.sessionMode = "inactive";
    mocks.requestedLiveTranscription = null;
    mocks.liveTranscriptionActive = null;
    mocks.toolbarControls.mockClear();
  });

  it("passes batch-only recording state to the chat session", () => {
    mocks.sessionMode = "active";
    mocks.requestedLiveTranscription = false;
    mocks.liveTranscriptionActive = false;

    render(<TestChatPanel />);

    expect(mocks.chatSession).toHaveBeenCalledWith(
      expect.objectContaining({
        hasAvailableTranscript: false,
        isBatchTranscriptionPending: true,
      }),
    );
  });

  it("preserves an existing transcript during batch retranscription", () => {
    mocks.hasAvailableTranscript = true;
    mocks.sessionMode = "active";
    mocks.requestedLiveTranscription = false;
    mocks.liveTranscriptionActive = false;

    render(<TestChatPanel />);

    expect(mocks.chatSession).toHaveBeenCalledWith(
      expect.objectContaining({
        hasAvailableTranscript: true,
        isBatchTranscriptionPending: true,
      }),
    );
  });

  it("passes the active folder into the chat session", () => {
    mocks.folderFilter = "CS 101";

    render(<TestChatPanel />);

    expect(mocks.chatSession).toHaveBeenCalledWith(
      expect.objectContaining({
        folderId: "CS 101",
      }),
    );
  });

  it("does not inherit note context in the automations scope", () => {
    mocks.chat.scope = "automations";
    mocks.folderFilter = "CS 101";
    mocks.hasAvailableTranscript = true;
    mocks.sessionMode = "active";

    const { container } = render(<TestChatPanel layout="right-panel" />);

    expect(mocks.chatSession).toHaveBeenCalledWith(
      expect.objectContaining({
        currentSessionId: undefined,
        folderId: undefined,
        hasAvailableTranscript: false,
        isBatchTranscriptionPending: false,
      }),
    );
    expect(screen.queryByTestId("chat-toolbar")).toBeNull();
    expect(mocks.toolbarControls).not.toHaveBeenCalled();
    expect(container.firstElementChild?.className).not.toContain("pb-3");
  });

  it("uses the sidebar card shell in the right panel layout", () => {
    const { container } = render(<TestChatPanel layout="right-panel" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-card");
    expect(root?.className).toContain("text-card-foreground");
    expect(root?.className).toContain("h-full");
    expect(root?.className).not.toContain("bg-primary");
    expect(root?.firstElementChild?.className).toContain("h-9");
    expect(root?.firstElementChild?.className).not.toContain("border-b");
    expect(
      root?.firstElementChild?.hasAttribute("data-tauri-drag-region"),
    ).toBe(true);
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("light");
    expect(mocks.toolbarControls).toHaveBeenCalledWith(
      expect.objectContaining({
        layout: "right-panel",
        onClose: expect.any(Function),
        surface: "light",
      }),
    );
  });

  it("uses the neutral shell in the floating layout", () => {
    const { container } = render(<TestChatPanel layout="floating" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-[#f4f4f5]");
    expect(root?.className).toContain("text-card-foreground");
    expect(root?.className).toContain("max-h-full");
    expect(root?.className).not.toContain("bg-card");
    expect(root?.className.split(" ")).not.toContain("h-full");
    expect(root?.firstElementChild?.className).toContain("h-11");
    expect(root?.firstElementChild?.className).not.toContain("border-b");
    expect(
      root?.firstElementChild?.hasAttribute("data-tauri-drag-region"),
    ).toBe(false);
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("light");
  });
});
