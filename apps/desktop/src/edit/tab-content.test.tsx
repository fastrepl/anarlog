import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionProposal: vi.fn(),
  applySessionProposal: vi.fn(),
  declineSessionProposal: vi.fn(),
  close: vi.fn(),
  tabs: [] as Array<Record<string, unknown>>,
  useSessionSummary: vi.fn(),
  useEnhancedNote: vi.fn(),
}));

vi.mock("@pierre/diffs/react", () => ({
  MultiFileDiff: () => <div data-testid="diff" />,
}));

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/session/queries", () => ({
  loadSessionProposal: mocks.loadSessionProposal,
  applySessionProposal: mocks.applySessionProposal,
  declineSessionProposal: mocks.declineSessionProposal,
  useSessionSummary: mocks.useSessionSummary,
  useEnhancedNote: mocks.useEnhancedNote,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => ({
      tabs: mocks.tabs,
      close: mocks.close,
    }),
  },
}));

import { TabContentEdit } from "./tab-content";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

const tab = {
  type: "edit" as const,
  requestId: "proposal-1",
  active: true,
  pinned: false,
  slotId: "slot-1",
};

function renderTab() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <TabContentEdit tab={tab} />
    </QueryClientProvider>,
  );
}

describe("TabContentEdit", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    mocks.tabs = [tab];
    mocks.applySessionProposal.mockResolvedValue(undefined);
    mocks.declineSessionProposal.mockResolvedValue(undefined);
    mocks.useSessionSummary.mockReturnValue({ title: "Planning" });
    mocks.useEnhancedNote.mockReturnValue({ title: "Summary" });
    mocks.loadSessionProposal.mockResolvedValue({
      id: "proposal-1",
      sessionId: "session-1",
      kind: "summary_replace",
      targetId: "summary-1",
      currentMarkdown: "Current",
      proposedMarkdown: "Proposed",
      status: "pending",
      source: "cli",
    });
  });

  it("loads a CLI proposal from the database and applies it", async () => {
    renderTab();

    expect(await screen.findByText("Planning")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Apply to summary" }));

    await waitFor(() => {
      expect(mocks.applySessionProposal).toHaveBeenCalledWith("proposal-1");
    });
    expect(mocks.close).toHaveBeenCalledWith(tab);
  });

  it("declines a database-backed proposal without auto-writing", async () => {
    renderTab();

    fireEvent.click(await screen.findByRole("button", { name: "Decline" }));

    await waitFor(() => {
      expect(mocks.declineSessionProposal).toHaveBeenCalledWith("proposal-1");
    });
    expect(mocks.applySessionProposal).not.toHaveBeenCalled();
  });

  it("shows a stale apply error without closing the review", async () => {
    mocks.applySessionProposal.mockRejectedValueOnce(
      new Error(
        "This proposal is stale. The meeting changed after it was created.",
      ),
    );
    renderTab();

    fireEvent.click(
      await screen.findByRole("button", { name: "Apply to summary" }),
    );

    expect(
      await screen.findByText(
        "This proposal is stale. The meeting changed after it was created.",
      ),
    ).toBeTruthy();
    expect(mocks.close).not.toHaveBeenCalled();
  });
});
