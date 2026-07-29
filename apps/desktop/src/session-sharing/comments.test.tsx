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
  captureCommentAnchor: vi.fn(() => ({
    quoteExact: "selected text",
    quotePrefix: "before ",
    quoteSuffix: " after",
    fromHint: 2,
    toHint: 15,
    snapshotRevision: 3,
  })),
  createComment: vi.fn(),
  deleteComment: vi.fn(),
  getCommentAnchorRanges: vi.fn<
    () => Array<{ commentId: string; from: number; to: number }>
  >(() => []),
  listComments: vi.fn(),
  loadManagedShare: vi.fn(),
  setActiveCommentAnchor: vi.fn(),
  setCommentAnchors: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: { user: { id: "viewer-1", is_anonymous: false } },
    supabase: {},
  }),
}));

vi.mock("@anlg/editor/comments", () => ({
  captureCommentAnchor: mocks.captureCommentAnchor,
  resolveCommentAnchors: (
    _doc: unknown,
    comments: Array<{ commentId: string }>,
  ) => comments.map((comment) => ({ ...comment, range: { from: 2, to: 15 } })),
}));

vi.mock("@anlg/editor/note", () => ({
  getCommentAnchorRanges: mocks.getCommentAnchorRanges,
  setActiveCommentAnchor: mocks.setActiveCommentAnchor,
  setCommentAnchors: mocks.setCommentAnchors,
}));

vi.mock("./client", () => ({
  createSessionShareComment: mocks.createComment,
  deleteSessionShareComment: mocks.deleteComment,
  listSessionShareComments: mocks.listComments,
  ShareManagementError: class extends Error {},
}));

vi.mock("~/shared-notes/cache", () => ({
  loadManagedSharedNoteForSession: mocks.loadManagedShare,
}));

import {
  SessionCommentsLayer,
  useOwnedSessionComments,
  useSharedSessionComments,
} from "./comments";

const shareId = "22222222-2222-4222-8222-222222222222";
const commentId = "99999999-9999-4999-8999-999999999999";

function Harness({ canCompose }: { canCompose: boolean }) {
  const controller = useSharedSessionComments({
    canCompose,
    currentRevision: 3,
    manageAccess: false,
    shareId,
  });
  const editorView = {
    state: { doc: {} },
    coordsAtPos: () => ({ bottom: 60, left: 80, right: 120, top: 40 }),
    dispatch: vi.fn(),
  } as any;

  return (
    <div ref={controller.containerRef}>
      <button type="button" onClick={() => controller.onViewReady(editorView)}>
        Mount editor
      </button>
      <button
        type="button"
        onClick={() =>
          controller.onCommentAnchorsEvent({
            type: "selection",
            empty: false,
            from: 2,
            to: 15,
          })
        }
      >
        Select text
      </button>
      <button
        type="button"
        onClick={() =>
          controller.onCommentAnchorsEvent({
            type: "anchor-click",
            commentIds: [commentId],
            pos: 2,
          })
        }
      >
        Open comment
      </button>
      <button
        type="button"
        onClick={() =>
          controller.onCommentAnchorsEvent({
            type: "anchor-click",
            commentIds: ["draft"],
            pos: 2,
          })
        }
      >
        Open draft highlight
      </button>
      <button
        type="button"
        onClick={() => controller.onCommentAnchorsEvent({ type: "layout" })}
      >
        Reposition comments
      </button>
      <SessionCommentsLayer controller={controller} />
    </div>
  );
}

function renderHarness(canCompose: boolean) {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return render(<Harness canCompose={canCompose} />, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
}

function OwnedSummaryHarness() {
  const controller = useOwnedSessionComments("session-1");
  const editorView = {
    state: { doc: {} },
    coordsAtPos: () => ({ bottom: 60, left: 80, right: 120, top: 40 }),
    dispatch: vi.fn(),
  } as any;

  return (
    <div ref={controller.containerRef}>
      <button type="button" onClick={() => controller.onViewReady(editorView)}>
        Mount owner editor
      </button>
      <button
        type="button"
        onClick={() =>
          controller.onCommentAnchorsEvent({
            type: "selection",
            empty: false,
            from: 2,
            to: 15,
          })
        }
      >
        Select owner text
      </button>
      <SessionCommentsLayer controller={controller} />
    </div>
  );
}

function renderOwnedSummary() {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return render(<OwnedSummaryHarness />, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
}

describe("desktop shared-note comments", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listComments.mockResolvedValue({ comments: [], nextCursor: null });
    mocks.loadManagedShare.mockResolvedValue({
      shareId,
      sessionId: "session-1",
      workspaceId: "workspace-1",
      contentRevision: 3,
    });
    mocks.createComment.mockResolvedValue({
      commentId,
      isAuthor: true,
      snapshotContentRevision: 3,
      body: "Follow up",
      anchor: {
        quoteExact: "selected text",
        quotePrefix: "before ",
        quoteSuffix: " after",
        fromHint: 2,
        toHint: 15,
      },
      createdAt: "2026-07-22T12:00:00.000Z",
    });
  });

  afterEach(cleanup);

  it("creates an anchored comment from selected summary text", async () => {
    renderHarness(true);

    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Select text" }));
    fireEvent.click(screen.getByRole("button", { name: "Comment" }));
    fireEvent.change(
      await screen.findByRole("textbox", { name: "Comment on selected text" }),
      { target: { value: "  Follow up  " } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Comment" }));

    await waitFor(() =>
      expect(mocks.createComment).toHaveBeenCalledWith(expect.anything(), {
        shareId,
        body: "Follow up",
        anchor: {
          quoteExact: "selected text",
          quotePrefix: "before ",
          quoteSuffix: " after",
          fromHint: 2,
          toHint: 15,
        },
      }),
    );
  });

  it("enables comments inside the owner's shared meeting summary", async () => {
    renderOwnedSummary();

    await waitFor(() => expect(mocks.loadManagedShare).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Mount owner editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Select owner text" }));

    expect(await screen.findByRole("button", { name: "Comment" })).toBeTruthy();
    expect(mocks.captureCommentAnchor).toHaveBeenCalledWith(
      expect.anything(),
      2,
      15,
      3,
    );
  });

  it("does not submit the same draft twice while creation is pending", async () => {
    mocks.createComment.mockReturnValue(new Promise(() => {}));
    renderHarness(true);

    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Select text" }));
    fireEvent.click(screen.getByRole("button", { name: "Comment" }));
    const textbox = await screen.findByRole("textbox", {
      name: "Comment on selected text",
    });
    fireEvent.change(textbox, { target: { value: "Follow up" } });
    fireEvent.keyDown(textbox, { key: "Enter", metaKey: true });
    await waitFor(() =>
      expect(
        (screen.getByRole("button", { name: "Comment" }) as HTMLButtonElement)
          .disabled,
      ).toBe(true),
    );
    fireEvent.keyDown(textbox, { key: "Enter", metaKey: true });

    expect(mocks.createComment).toHaveBeenCalledTimes(1);
  });

  it("keeps an unsaved draft open when its highlight is clicked", async () => {
    renderHarness(true);

    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Select text" }));
    fireEvent.click(screen.getByRole("button", { name: "Comment" }));
    const textbox = await screen.findByRole("textbox", {
      name: "Comment on selected text",
    });
    fireEvent.change(textbox, { target: { value: "Unsaved thought" } });
    fireEvent.click(
      screen.getByRole("button", { name: "Open draft highlight" }),
    );

    expect(screen.getByDisplayValue("Unsaved thought")).toBeTruthy();
  });

  it("keeps the draft highlight mapped through editor changes", async () => {
    renderHarness(true);

    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Select text" }));
    fireEvent.click(screen.getByRole("button", { name: "Comment" }));
    mocks.getCommentAnchorRanges.mockReturnValue([
      { commentId: "draft", from: 4, to: 18 },
    ]);
    fireEvent.click(
      screen.getByRole("button", { name: "Reposition comments" }),
    );

    await waitFor(() =>
      expect(mocks.setCommentAnchors).toHaveBeenLastCalledWith(
        expect.anything(),
        [expect.objectContaining({ commentId: "draft", from: 4, to: 18 })],
      ),
    );
  });

  it("keeps viewer-only summaries read-only for comments", () => {
    renderHarness(false);

    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Select text" }));

    expect(screen.queryByRole("button", { name: "Comment" })).toBeNull();
    expect(mocks.createComment).not.toHaveBeenCalled();
  });

  it("opens existing anchored comments for every authorized viewer", async () => {
    mocks.listComments.mockResolvedValue({
      comments: [
        {
          commentId,
          isAuthor: false,
          snapshotContentRevision: 3,
          body: "Please clarify this decision",
          anchor: {
            quoteExact: "selected text",
            quotePrefix: "before ",
            quoteSuffix: " after",
            fromHint: 2,
            toHint: 15,
          },
          createdAt: "2026-07-22T12:00:00.000Z",
        },
      ],
      nextCursor: null,
    });
    renderHarness(false);

    await waitFor(() => expect(mocks.listComments).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    fireEvent.click(screen.getByRole("button", { name: "Open comment" }));

    expect(
      await screen.findByText("Please clarify this decision"),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Delete comment" })).toBeNull();
  });
});
