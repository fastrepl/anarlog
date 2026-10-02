import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: { user: { id: "viewer-1" } } as any,
  query: {
    data: null as any,
    isLoading: false,
    error: null as Error | null,
  },
  preview: { status: "unavailable" } as any,
  signIn: vi.fn(),
  startCommentDraft: vi.fn(),
  commentInputs: [] as Array<Record<string, unknown>>,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session, signIn: mocks.signIn }),
}));

vi.mock("~/shared-notes/cache", () => ({
  useDurableSharedNote: () => mocks.query,
}));

vi.mock("~/shared-notes/preview", () => ({
  useSharedNotePreview: () => mocks.preview,
}));

vi.mock("~/shared-notes/use-shared-attachment-resolver", () => ({
  useSharedAttachmentResolver: () => () => null,
}));

vi.mock("~/session-sharing/comments", () => ({
  SessionCommentsLayer: () => <div data-testid="shared-comments-layer" />,
  useSharedSessionComments: (input: Record<string, unknown>) => {
    mocks.commentInputs.push(input);
    return {
      containerRef: { current: null },
      onCommentAnchorsEvent: vi.fn(),
      onViewDisposed: vi.fn(),
      onViewReady: vi.fn(),
      draft: null,
      selection: input.canCompose ? {} : null,
      startDraft: mocks.startCommentDraft,
    };
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openPath: vi.fn() },
}));

vi.mock("~/session/components/session-surface", () => ({
  SessionSurface: ({
    header,
    children,
  }: {
    header?: ReactNode;
    children: ReactNode;
  }) => (
    <div>
      {header}
      {children}
    </div>
  ),
}));

vi.mock("@anlg/editor/note", () => ({
  NoteEditor: ({
    readOnly,
    initialContent,
    onCommentSelection,
  }: {
    readOnly?: boolean;
    initialContent?: unknown;
    onCommentSelection?: () => void;
  }) => (
    <div
      data-content={JSON.stringify(initialContent)}
      data-read-only={String(readOnly)}
      data-comment-action={String(Boolean(onCommentSelection))}
      data-testid="shared-note-editor"
    />
  ),
}));

vi.mock("~/editor-bridge/open-editor-link", () => ({
  openEditorLink: vi.fn(),
}));

import { TabContentSharedNote, TabContentSharedNotePreview } from ".";

const tab = {
  type: "shared_sessions" as const,
  id: "share-1",
  active: true,
  slotId: "slot-1",
  pinned: false,
};

function sharedSnapshot(overrides: Record<string, unknown> = {}) {
  return {
    shareId: "share-1",
    workspaceId: "workspace-1",
    sessionId: "session-1",
    schemaVersion: 1,
    contentRevision: 2,
    title: "Shared plan",
    body: { type: "doc", content: [{ type: "paragraph" }] },
    attachments: [],
    capability: "viewer",
    manageAccess: false,
    accessVersion: 3,
    publishedAt: "2026-07-16T17:30:00.000Z",
    ...overrides,
  };
}

function renderSharedNote() {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return render(<TabContentSharedNote tab={tab} />, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
}

describe("TabContentSharedNote", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.session = { user: { id: "viewer-1" } };
    mocks.query = { data: null, isLoading: false, error: null };
    mocks.preview = { status: "unavailable" };
    mocks.signIn.mockResolvedValue(undefined);
    mocks.commentInputs = [];
  });

  it("renders a handoff preview without durable identifiers", () => {
    mocks.preview = {
      status: "ready",
      snapshot: {
        shareId: "f733dd21-336b-4b99-8967-c1e05509268e",
        schemaVersion: 1,
        contentRevision: 1,
        title: "Public plan",
        body: { type: "doc", content: [{ type: "paragraph" }] },
        attachments: [],
        attachmentDownloads: [],
        publishedAt: "2026-07-17T10:00:00.000Z",
      },
    };

    render(
      <TabContentSharedNotePreview
        tab={{
          type: "shared_note_preview",
          id: "13697a87-f69b-456d-8679-4202d4f5d498",
          active: true,
          slotId: "slot-preview",
          pinned: false,
        }}
      />,
    );

    expect(screen.getByText("Shared link · View only")).toBeTruthy();
    expect(
      screen.getByTestId("shared-note-editor").getAttribute("data-read-only"),
    ).toBe("true");
  });

  afterEach(cleanup);

  it.each([
    {
      capability: "viewer",
      label: "Shared with me · View only",
      canCompose: false,
      commentAction: "false",
    },
    {
      capability: "commenter",
      label: "Shared with me · Can comment",
      canCompose: true,
      commentAction: "true",
    },
  ])(
    "renders a $capability shared note with a read-only editor",
    ({ capability, label, canCompose, commentAction }) => {
      mocks.query.data = sharedSnapshot({ capability });

      renderSharedNote();

      const editor = screen.getByTestId("shared-note-editor");
      expect(screen.getByText(label)).toBeTruthy();
      expect(editor.getAttribute("data-read-only")).toBe("true");
      expect(editor.dataset.content).toContain("Shared plan");
      expect(mocks.commentInputs[mocks.commentInputs.length - 1]).toMatchObject(
        {
          canCompose,
          manageAccess: false,
          shareId: "share-1",
        },
      );
      expect(editor.dataset.commentAction).toBe(commentAction);
    },
  );

  it("removes content when access is no longer cached", () => {
    renderSharedNote();

    expect(screen.getByText("Access no longer available")).toBeTruthy();
    expect(screen.queryByTestId("shared-note-editor")).toBeNull();
  });

  it.each([null, { user: { id: "anonymous-1", is_anonymous: true } }])(
    "requires sign-in for session %s",
    async (session) => {
      mocks.session = session;

      renderSharedNote();

      expect(screen.getByText("Sign in to view this shared note")).toBeTruthy();
      expect(screen.queryByText("Access no longer available")).toBeNull();
      fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
      await waitFor(() => expect(mocks.signIn).toHaveBeenCalledTimes(1));
    },
  );
});
