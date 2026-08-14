import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionContentSnapshot: vi.fn(),
  updateSession: vi.fn(),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/session/queries", () => ({
  updateSession: mocks.updateSession,
}));

import { buildEditMemoTool } from "./edit-memo";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

describe("edit memo chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    mocks.updateSession.mockResolvedValue(undefined);
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      rawMarkdown: "Existing notes",
    });
  });

  it("creates meeting preparation in an empty memo after review", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue({ rawMarkdown: "" });
    const openEditTab = vi.fn((requestId: string) => {
      expect(usePendingEditStore.getState().edits.get(requestId)).toMatchObject(
        {
          sessionId: "session-1",
          target: { kind: "memo" },
          currentContent: "",
          proposedContent: "## Agenda\n\n- Review blockers",
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

    expect(openEditTab).toHaveBeenCalledWith("request-1");
    expect(mocks.updateSession).toHaveBeenCalledWith("session-1", {
      raw_md: expect.stringContaining("Review blockers"),
    });
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

    expect(mocks.updateSession).not.toHaveBeenCalled();
  });
});
