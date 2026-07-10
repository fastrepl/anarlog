import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionContentSnapshot: vi.fn(),
  loadHumansByIds: vi.fn(),
  buildRenderTranscriptRequestFromRows: vi.fn(),
  collectAssignedHumanIdsFromTranscriptRows: vi.fn(),
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("~/contacts/queries", () => ({
  loadHumansByIds: mocks.loadHumansByIds,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/stt/render-transcript", () => ({
  buildRenderTranscriptRequestFromRows:
    mocks.buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows:
    mocks.collectAssignedHumanIdsFromTranscriptRows,
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

import { hydrateSessionContext } from "./session-context-hydrator";

describe("session chat context hydration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.collectAssignedHumanIdsFromTranscriptRows.mockReturnValue([
      "human-assigned",
    ]);
    mocks.loadHumansByIds.mockResolvedValue([
      { id: "human-1", name: "SQLite Person", jobTitle: "Engineer" },
      { id: "human-assigned", name: "Assigned Person", jobTitle: "" },
      { id: "user-1", name: "Self", jobTitle: "" },
    ]);
    mocks.buildRenderTranscriptRequestFromRows.mockReturnValue({
      transcripts: [],
      participant_human_ids: [],
      self_human_id: "user-1",
      humans: [],
    });
    mocks.renderTranscriptSegments.mockResolvedValue([
      { speaker_label: "SQLite Person", text: "Transcript text" },
    ]);
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      sessionId: "session-1",
      title: "Planning",
      createdAt: "2026-07-10T09:00:00.000Z",
      event: { title: "Weekly planning" },
      eventId: "event-1",
      rawMarkdown: "Raw note",
      enhancedNotes: [
        {
          id: "summary-1",
          title: "First",
          markdown: "First",
          position: 1,
        },
        {
          id: "summary-2",
          title: "Later",
          markdown: "Second",
          position: 2,
        },
      ],
      transcripts: [
        {
          id: "transcript-1",
          started_at: 100,
          ended_at: 200,
          memo: "",
          words: [
            {
              id: "word-1",
              text: "Transcript text",
              start_ms: 0,
              end_ms: 100,
            },
          ],
          speaker_hints: [],
        },
      ],
      participants: [
        {
          humanId: "human-1",
          name: "SQLite Person",
          jobTitle: "Engineer",
        },
      ],
    });
  });

  it("hydrates note and speaker context from the canonical snapshot", async () => {
    await expect(hydrateSessionContext("session-1", "user-1")).resolves.toEqual(
      {
        title: "Planning",
        date: "2026-07-10T09:00:00.000Z",
        rawContent: "Raw note",
        enhancedContent: "First\n\n---\n\nSecond",
        transcript: {
          segments: [{ speaker: "SQLite Person", text: "Transcript text" }],
          startedAt: 100,
          endedAt: 200,
        },
        participants: [{ name: "SQLite Person", jobTitle: "Engineer" }],
        event: { name: "Weekly planning" },
      },
    );

    expect(mocks.loadHumansByIds).toHaveBeenCalledWith([
      "human-1",
      "human-assigned",
      "user-1",
    ]);
    expect(mocks.buildRenderTranscriptRequestFromRows).toHaveBeenCalledWith(
      expect.any(Array),
      expect.objectContaining({
        selfHumanId: "user-1",
        humans: expect.arrayContaining([
          { human_id: "human-assigned", name: "Assigned Person" },
        ]),
      }),
      ["human-1"],
    );
  });

  it("returns null when the canonical session is unavailable", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValueOnce(null);

    await expect(
      hydrateSessionContext("session-missing", "user-1"),
    ).resolves.toBeNull();
    expect(mocks.loadHumansByIds).not.toHaveBeenCalled();
  });
});
