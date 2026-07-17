import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openCurrent: vi.fn(),
  currentTab: null as { type: string; id?: string } | null,
  notes: [] as Array<{
    shareId: string;
    sessionId: string;
    title: string;
    manageAccess: boolean;
  }>,
  sessions: [] as Array<{ id: string }>,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "viewer-1" } } }),
}));

vi.mock("~/shared-notes/cache", () => ({
  useDurableSharedNotes: () => mocks.notes,
}));

vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => mocks.sessions,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      currentTab: typeof mocks.currentTab;
      openCurrent: typeof mocks.openCurrent;
    }) => unknown,
  ) =>
    selector({
      currentTab: mocks.currentTab,
      openCurrent: mocks.openCurrent,
    }),
}));

import { SharedNotesNav } from "./shared-notes";

describe("SharedNotesNav", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.currentTab = null;
    mocks.notes = [];
    mocks.sessions = [];
  });

  afterEach(cleanup);

  it("shows accessible shared notes with a dedicated people affordance", () => {
    mocks.notes = [
      {
        shareId: "share-1",
        sessionId: "remote-session",
        title: "Shared plan",
        manageAccess: false,
      },
      {
        shareId: "owned-share",
        sessionId: "local-session",
        title: "Owned",
        manageAccess: true,
      },
      {
        shareId: "admin-share",
        sessionId: "remote-admin-session",
        title: "Admin remote",
        manageAccess: true,
      },
      {
        shareId: "viewer-local-share",
        sessionId: "local-session",
        title: "Viewer local",
        manageAccess: false,
      },
    ];
    mocks.sessions = [{ id: "local-session" }];

    render(<SharedNotesNav />);

    expect(screen.getByText("Shared with me")).toBeTruthy();
    expect(screen.getByText("Shared plan")).toBeTruthy();
    expect(screen.queryByText("Owned")).toBeNull();
    expect(screen.getByText("Admin remote")).toBeTruthy();
    expect(screen.getByText("Viewer local")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Shared plan" }));
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "shared_sessions",
      id: "share-1",
    });
  });
});
