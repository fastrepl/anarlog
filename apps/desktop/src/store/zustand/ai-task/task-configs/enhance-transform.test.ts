import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { enhanceTransform } from "./enhance-transform";

const mocks = vi.hoisted(() => ({
  collectEnhanceImageContext: vi.fn(),
  getTemplateById: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  loadHumansByIds: vi.fn(),
  buildRenderTranscriptRequestFromRows: vi.fn(),
  collectAssignedHumanIdsFromTranscriptRows: vi.fn(),
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("./enhance-images", () => ({
  collectEnhanceImageContext: mocks.collectEnhanceImageContext,
}));

vi.mock("~/templates/queries", () => ({
  getTemplateById: mocks.getTemplateById,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/contacts/queries", () => ({
  loadHumansByIds: mocks.loadHumansByIds,
}));

vi.mock("~/stt/render-transcript", () => ({
  buildRenderTranscriptRequestFromRows:
    mocks.buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows:
    mocks.collectAssignedHumanIdsFromTranscriptRows,
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

function createSnapshot() {
  return {
    sessionId: "session-1",
    ownerUserId: "user-1",
    title: "Weekly Review",
    createdAt: "2026-07-10T00:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: "session-1",
    rawContent: "![post](asset://localhost/post.png)",
    rawContentFormat: "markdown",
    rawMarkdown: "![post](asset://localhost/post.png)",
    enhancedNotes: [],
    transcripts: [
      {
        id: "transcript-1",
        started_at: 100,
        ended_at: 200,
        memo: "![pre](asset://localhost/pre.png)",
        wordsJson: "[]",
        words: [],
        speaker_hints: [],
      },
    ],
    participants: [{ humanId: "human-1", name: "Alice", jobTitle: "Engineer" }],
  };
}

const settingsValues = { ai_language: "en" } as const;

describe("enhanceTransform.transformArgs", () => {
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.collectEnhanceImageContext.mockResolvedValue([]);
    mocks.getTemplateById.mockResolvedValue(null);
    mocks.loadSessionContentSnapshot.mockResolvedValue(createSnapshot());
    mocks.loadHumansByIds.mockResolvedValue([{ id: "human-1", name: "Alice" }]);
    mocks.collectAssignedHumanIdsFromTranscriptRows.mockReturnValue([]);
    mocks.buildRenderTranscriptRequestFromRows.mockReturnValue(null);
    mocks.renderTranscriptSegments.mockResolvedValue([]);
    consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleError.mockRestore();
  });

  it("uses the selected template when it can be loaded", async () => {
    mocks.getTemplateById.mockResolvedValue({
      title: "Standup",
      description: "Daily sync",
      sections: [{ title: "Updates", description: null }],
    });

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      settingsValues,
    );

    expect(result.template).toEqual({
      title: "Standup",
      description: "Daily sync",
      sections: [{ title: "Updates", description: null }],
    });
    expect(result.participants).toEqual([
      { name: "Alice", jobTitle: "Engineer" },
    ]);
  });

  it("falls back to generic enhancement when template loading fails", async () => {
    mocks.getTemplateById.mockRejectedValue(new Error("Failed query"));

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      settingsValues,
    );

    expect(result.template).toBeNull();
    expect(result.session.title).toBe("Weekly Review");
    expect(consoleError).toHaveBeenCalledWith(
      "[enhance] failed to load template",
      expect.any(Error),
    );
  });

  it("collects image context from canonical transcript and note content", async () => {
    await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
      {
        current_llm_provider: "openai",
        current_llm_model: "gpt-4o",
        ai_language: "en",
      },
    );

    expect(mocks.collectEnhanceImageContext).toHaveBeenCalledWith("session-1", [
      "![pre](asset://localhost/pre.png)",
      "![post](asset://localhost/post.png)",
    ]);
  });

  it("builds speaker identity context from SQLite humans", async () => {
    mocks.collectAssignedHumanIdsFromTranscriptRows.mockReturnValue([
      "human-2",
    ]);

    await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      settingsValues,
    );

    expect(mocks.loadHumansByIds).toHaveBeenCalledWith([
      "user-1",
      "human-1",
      "human-2",
    ]);
    expect(mocks.buildRenderTranscriptRequestFromRows).toHaveBeenCalledWith(
      expect.any(Array),
      {
        selfHumanId: "user-1",
        humans: [{ human_id: "human-1", name: "Alice" }],
      },
      ["human-1"],
    );
  });

  it("rejects generation when the session no longer exists", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(null);

    await expect(
      enhanceTransform.transformArgs(
        { sessionId: "missing", enhancedNoteId: "note-1" },
        settingsValues,
      ),
    ).rejects.toThrow("Session missing no longer exists");
  });
});
