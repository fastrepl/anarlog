import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openCurrent: vi.fn(),
  onOpenChange: vi.fn(),
  notes: [] as Array<{
    shareId: string;
    sessionId: string;
    title: string;
    publishedAt: string;
    manageAccess: boolean;
  }>,
  sessions: [] as Array<{
    id: string;
    title: string;
    created_at: string;
  }>,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "viewer-1" } } }),
}));

vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => mocks.sessions,
}));

vi.mock("~/shared-notes/cache", () => ({
  useDurableSharedNotes: () => mocks.notes,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      openCurrent: typeof mocks.openCurrent;
      recentlyOpenedSessionIds: string[];
    }) => unknown,
  ) =>
    selector({
      openCurrent: mocks.openCurrent,
      recentlyOpenedSessionIds: [],
    }),
}));

import { OpenNoteDialog } from "./open-note-dialog";

describe("OpenNoteDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.notes = [];
    mocks.sessions = [];
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
    Element.prototype.scrollIntoView = vi.fn();
  });

  afterEach(cleanup);

  it("opens a durable shared note from All Notes", () => {
    mocks.notes = [
      {
        shareId: "share-1",
        sessionId: "remote-session",
        title: "Shared roadmap",
        publishedAt: "2026-07-16T09:00:00.000Z",
        manageAccess: false,
      },
      {
        shareId: "owned-share",
        sessionId: "local-session",
        title: "Owned note",
        publishedAt: "2026-07-15T09:00:00.000Z",
        manageAccess: true,
      },
      {
        shareId: "viewer-local-share",
        sessionId: "local-session",
        title: "Viewer local snapshot",
        publishedAt: "2026-07-14T09:00:00.000Z",
        manageAccess: false,
      },
    ];
    mocks.sessions = [
      {
        id: "local-session",
        title: "Owned canonical note",
        created_at: "2026-07-15T09:00:00.000Z",
      },
    ];

    render(<OpenNoteDialog open onOpenChange={mocks.onOpenChange} />);

    expect(screen.getByText("All Notes")).toBeTruthy();
    const sharedNote = screen.getByRole("option", {
      name: "Shared roadmap",
    });
    expect(sharedNote.querySelector(".lucide-users-round")).toBeTruthy();
    expect(screen.queryByText("Owned note")).toBeNull();
    expect(screen.getByText("Owned canonical note")).toBeTruthy();
    expect(screen.getByText("Viewer local snapshot")).toBeTruthy();

    fireEvent.click(sharedNote);

    expect(mocks.onOpenChange).toHaveBeenCalledWith(false);
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "shared_sessions",
      id: "share-1",
    });
  });
});
