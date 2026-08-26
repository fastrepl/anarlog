import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  applySessionProposal: vi.fn(),
  declineSessionProposal: vi.fn(),
  close: vi.fn(),
  openNew: vi.fn(),
  tabs: [] as Array<Record<string, unknown>>,
  invalidateQueries: vi.fn(),
}));

vi.mock("~/session/queries", () => ({
  applySessionProposal: mocks.applySessionProposal,
  declineSessionProposal: mocks.declineSessionProposal,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => ({
      tabs: mocks.tabs,
      close: mocks.close,
      openNew: mocks.openNew,
    }),
  },
}));

import {
  applyProposalReview,
  closeProposalReviewTab,
  declineProposalReview,
  openProposalReview,
  proposalKindLabel,
  shouldAutoDeclineProposal,
} from "./proposal-review";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

describe("proposal review helpers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    mocks.tabs = [
      {
        type: "edit",
        requestId: "proposal-1",
        slotId: "slot-1",
      },
    ];
    mocks.applySessionProposal.mockResolvedValue(undefined);
    mocks.declineSessionProposal.mockResolvedValue(undefined);
  });

  it("does not auto-decline CLI or MCP proposals", () => {
    expect(shouldAutoDeclineProposal("chat")).toBe(true);
    expect(shouldAutoDeclineProposal(undefined)).toBe(true);
    expect(shouldAutoDeclineProposal("cli")).toBe(false);
    expect(shouldAutoDeclineProposal("mcp")).toBe(false);
  });

  it("labels memo and summary proposal kinds", () => {
    expect(proposalKindLabel("memo_replace")).toBe("memo");
    expect(proposalKindLabel("summary_replace")).toBe("summary");
  });

  it("opens and closes the review tab by request id", () => {
    openProposalReview("proposal-1");
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "edit",
      requestId: "proposal-1",
    });

    closeProposalReviewTab("proposal-1");
    expect(mocks.close).toHaveBeenCalledWith(mocks.tabs[0]);
  });

  it("applies, resolves waiters, and closes the review tab", async () => {
    const resolve = vi.fn();
    usePendingEditStore.getState().addEdit({
      requestId: "proposal-1",
      sessionId: "session-1",
      target: { kind: "memo" },
      currentContent: "",
      proposedContent: "Agenda",
      source: "chat",
      resolve,
    });

    await applyProposalReview("proposal-1", {
      invalidateQueries: mocks.invalidateQueries,
    } as never);

    expect(mocks.applySessionProposal).toHaveBeenCalledWith("proposal-1");
    expect(resolve).toHaveBeenCalledWith(true);
    expect(mocks.close).toHaveBeenCalledWith(mocks.tabs[0]);
    expect(mocks.invalidateQueries).toHaveBeenCalledWith({
      queryKey: ["session-proposals"],
    });
    expect(usePendingEditStore.getState().edits.has("proposal-1")).toBe(false);
  });

  it("declines without applying the meeting write", async () => {
    const resolve = vi.fn();
    usePendingEditStore.getState().addEdit({
      requestId: "proposal-1",
      sessionId: "session-1",
      target: { kind: "memo" },
      currentContent: "",
      proposedContent: "Agenda",
      source: "cli",
      resolve,
    });

    await declineProposalReview("proposal-1");

    expect(mocks.declineSessionProposal).toHaveBeenCalledWith("proposal-1");
    expect(resolve).toHaveBeenCalledWith(false);
    expect(mocks.applySessionProposal).not.toHaveBeenCalled();
  });
});
