import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionContentSnapshot: vi.fn(),
  updateEnhancedNoteContent: vi.fn(),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/session/queries", () => ({
  updateEnhancedNoteContent: mocks.updateEnhancedNoteContent,
}));

vi.mock("~/shared/utils", () => ({
  id: () => "request-1",
}));

import { buildEditSummaryTool } from "./edit-summary";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

describe("edit summary chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    mocks.updateEnhancedNoteContent.mockResolvedValue(undefined);
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

  it("awaits the reviewed SQLite summary write", async () => {
    const openEditTab = vi.fn((requestId: string) => {
      const pending = usePendingEditStore.getState().edits.get(requestId);
      expect(pending).toMatchObject({
        sessionId: "session-1",
        enhancedNoteId: "summary-1",
        currentContent: "Current summary",
        proposedContent: "Updated summary",
      });
      usePendingEditStore.getState().resolveEdit(requestId, true);
    });
    const editTool = buildEditSummaryTool({
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
      openEditTab,
    });

    await expect(
      (editTool as any).execute({ content: "Updated summary" }),
    ).resolves.toEqual({ status: "applied" });

    expect(openEditTab).toHaveBeenCalledWith("request-1");
    expect(mocks.updateEnhancedNoteContent).toHaveBeenCalledWith(
      "summary-1",
      "session-1",
      expect.stringContaining("Updated summary"),
    );
  });

  it("returns canonical candidates when the requested summary is unrelated", async () => {
    const editTool = buildEditSummaryTool({
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
      openEditTab: vi.fn(),
    });

    await expect(
      (editTool as any).execute({
        enhancedNoteId: "summary-other",
        content: "Updated summary",
      }),
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
    expect(mocks.updateEnhancedNoteContent).not.toHaveBeenCalled();
  });
});
