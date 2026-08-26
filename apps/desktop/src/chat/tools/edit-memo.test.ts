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

import { buildEditMemoTool } from "./edit-memo";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

describe("edit memo chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    mocks.persistChatSessionProposal.mockResolvedValue(undefined);
    mocks.applySessionProposal.mockResolvedValue(undefined);
    mocks.declineSessionProposal.mockResolvedValue(undefined);
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      rawMarkdown: "Existing notes",
      rawNoteId: "session-1",
    });
  });

  it("persists meeting preparation and applies it after review", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      rawMarkdown: "",
      rawNoteId: "session-1",
    });
    const openEditTab = vi.fn((requestId: string) => {
      expect(usePendingEditStore.getState().edits.get(requestId)).toMatchObject(
        {
          sessionId: "session-1",
          target: { kind: "memo" },
          currentContent: "",
          proposedContent: "## Agenda\n\n- Review blockers",
          source: "chat",
        },
      );
      usePendingEditStore.getState().resolveEdit(requestId, true);
    });
    const editTool = buildEditMemoTool({
      getSessionId: () => "session-1",
      openEditTab,
    });

    await expect(
      (editTool as any).execute(
        { content: "## Agenda\n\n- Review blockers" },
        { toolCallId: "request-1", messages: [] },
      ),
    ).resolves.toEqual({ status: "applied" });

    expect(mocks.persistChatSessionProposal).toHaveBeenCalledWith({
      id: "request-1",
      sessionId: "session-1",
      kind: "memo_replace",
      targetId: "session-1",
      currentMarkdown: "",
      proposedMarkdown: "## Agenda\n\n- Review blockers",
    });
    expect(openEditTab).toHaveBeenCalledWith("request-1");
    expect(mocks.applySessionProposal).toHaveBeenCalledWith("request-1");
  });

  it("does not overwrite the memo when the review is declined", async () => {
    const editTool = buildEditMemoTool({
      getSessionId: () => "session-1",
      openEditTab: (requestId) => {
        usePendingEditStore.getState().resolveEdit(requestId, false);
      },
    });

    await expect(
      (editTool as any).execute(
        { content: "Replacement" },
        { toolCallId: "request-1", messages: [] },
      ),
    ).resolves.toEqual({ status: "declined" });

    expect(mocks.persistChatSessionProposal).toHaveBeenCalled();
    expect(mocks.declineSessionProposal).toHaveBeenCalledWith("request-1");
    expect(mocks.applySessionProposal).not.toHaveBeenCalled();
  });
});
