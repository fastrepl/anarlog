import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionContentSnapshot: vi.fn(),
  persistChatSessionProposal: vi.fn(),
  applySessionProposal: vi.fn(),
  declineSessionProposal: vi.fn(),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/session/queries", () => ({
  persistChatSessionProposal: mocks.persistChatSessionProposal,
  applySessionProposal: mocks.applySessionProposal,
  declineSessionProposal: mocks.declineSessionProposal,
}));

import { buildEditSummaryTool } from "./edit-summary";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

describe("edit summary chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    mocks.persistChatSessionProposal.mockResolvedValue(undefined);
    mocks.applySessionProposal.mockResolvedValue(undefined);
    mocks.declineSessionProposal.mockResolvedValue(undefined);
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      enhancedNotes: [
        {
          id: "summary-1",
          title: "Summary",
          markdown: "Current summary",
          templateId: "",
          position: 0,
        },
      ],
    });
  });

  it("persists a proposal and applies it after review", async () => {
    const openEditTab = vi.fn((requestId: string) => {
      const pending = usePendingEditStore.getState().edits.get(requestId);
      expect(pending).toMatchObject({
        sessionId: "session-1",
        target: { kind: "summary", enhancedNoteId: "summary-1" },
        currentContent: "Current summary",
        proposedContent: "Updated summary",
        source: "chat",
      });
      usePendingEditStore.getState().resolveEdit(requestId, true);
    });
    const editTool = buildEditSummaryTool({
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
      openEditTab,
    });

    await expect(
      (editTool as any).execute(
        { content: "Updated summary" },
        { toolCallId: "request-1", messages: [] },
      ),
    ).resolves.toEqual({ status: "applied" });

    expect(mocks.persistChatSessionProposal).toHaveBeenCalledWith({
      id: "request-1",
      sessionId: "session-1",
      kind: "summary_replace",
      targetId: "summary-1",
      currentMarkdown: "Current summary",
      proposedMarkdown: "Updated summary",
    });
    expect(openEditTab).toHaveBeenCalledWith("request-1");
    expect(mocks.applySessionProposal).toHaveBeenCalledWith("request-1");
  });

  it("declines the persisted proposal when review is rejected", async () => {
    const editTool = buildEditSummaryTool({
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
      openEditTab: (requestId) => {
        usePendingEditStore.getState().resolveEdit(requestId, false);
      },
    });

    await expect(
      (editTool as any).execute(
        { content: "Updated summary" },
        { toolCallId: "request-1", messages: [] },
      ),
    ).resolves.toEqual({ status: "declined" });

    expect(mocks.declineSessionProposal).toHaveBeenCalledWith("request-1");
    expect(mocks.applySessionProposal).not.toHaveBeenCalled();
  });

  it("returns canonical candidates when the requested summary is unrelated", async () => {
    const editTool = buildEditSummaryTool({
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
      openEditTab: vi.fn(),
    });

    await expect(
      (editTool as any).execute(
        {
          enhancedNoteId: "summary-other",
          content: "Updated summary",
        },
        { toolCallId: "request-1", messages: [] },
      ),
    ).resolves.toEqual({
      status: "error",
      message: "That summary does not belong to the target session.",
      candidates: [
        {
          enhancedNoteId: "summary-1",
          title: "Summary",
          position: 0,
        },
      ],
    });
    expect(mocks.persistChatSessionProposal).not.toHaveBeenCalled();
  });
});
