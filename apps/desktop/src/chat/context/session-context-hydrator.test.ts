import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionContent: vi.fn(),
  loadHumansByIds: vi.fn(),
  loadSessionParticipantHumanIds: vi.fn(),
  buildRenderTranscriptRequestFromRows: vi.fn(),
  collectAssignedHumanIdsFromTranscriptRows: vi.fn(),
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: { loadSessionContent: mocks.loadSessionContent },
}));

vi.mock("~/contacts/queries", () => ({
  loadHumansByIds: mocks.loadHumansByIds,
}));

vi.mock("~/session/queries", () => ({
  loadSessionParticipantHumanIds: mocks.loadSessionParticipantHumanIds,
}));

vi.mock("~/stt/render-transcript", () => ({
  buildRenderTranscriptRequestFromRows:
    mocks.buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows:
    mocks.collectAssignedHumanIdsFromTranscriptRows,
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

import { hydrateSessionContextFromFs } from "./session-context-hydrator";

describe("session chat context hydration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadSessionParticipantHumanIds.mockResolvedValue(["human-sql"]);
    mocks.collectAssignedHumanIdsFromTranscriptRows.mockReturnValue([
      "human-assigned",
    ]);
    mocks.loadHumansByIds.mockResolvedValue([
      { id: "human-sql", name: "SQLite Person", jobTitle: "Engineer" },
      { id: "human-legacy", name: "Legacy Person", jobTitle: "Founder" },
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
    mocks.loadSessionContent.mockResolvedValue({
      status: "ok",
      data: {
        meta: {
          title: "Planning",
          createdAt: "2026-07-10T09:00:00.000Z",
          event: { title: "Weekly planning" },
          participants: [{ humanId: "human-legacy" }],
        },
        rawMemoMarkdown: "Raw note",
        notes: [
          { title: "Later", markdown: "Second", position: 2 },
          { title: "First", markdown: "First", position: 1 },
        ],
        transcript: {
          transcripts: [
            {
              id: "transcript-1",
              started_at: 100,
              ended_at: 200,
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
        },
      },
    });
  });

  it("combines canonical and legacy identities without dropping speakers", async () => {
    await expect(
      hydrateSessionContextFromFs("session-1", "user-1"),
    ).resolves.toEqual({
      title: "Planning",
      date: "2026-07-10T09:00:00.000Z",
      rawContent: "Raw note",
      enhancedContent: "First\n\n---\n\nSecond",
      transcript: {
        segments: [{ speaker: "SQLite Person", text: "Transcript text" }],
        startedAt: 100,
        endedAt: 200,
      },
      participants: [
        { name: "SQLite Person", jobTitle: "Engineer" },
        { name: "Legacy Person", jobTitle: "Founder" },
      ],
      event: { name: "Weekly planning" },
    });

    expect(mocks.loadHumansByIds).toHaveBeenCalledWith([
      "human-sql",
      "human-legacy",
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
      ["human-sql", "human-legacy"],
    );
  });

  it("does not query SQLite when the legacy session cannot be loaded", async () => {
    mocks.loadSessionContent.mockResolvedValueOnce({
      status: "error",
      error: "missing",
    });

    await expect(
      hydrateSessionContextFromFs("session-missing", "user-1"),
    ).resolves.toBeNull();
    expect(mocks.loadSessionParticipantHumanIds).not.toHaveBeenCalled();
  });
});
