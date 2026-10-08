import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  formatMeetingChatRecordsAsMarkdown: vi.fn(),
  loadMeetingChatRecords: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  renderSessionTranscript: vi.fn(),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: { renderSessionTranscript: mocks.renderSessionTranscript },
}));

vi.mock("~/stt/meeting-chat-records", () => ({
  formatMeetingChatRecordsAsMarkdown: mocks.formatMeetingChatRecordsAsMarkdown,
  loadMeetingChatRecords: mocks.loadMeetingChatRecords,
}));

import { hydrateSessionContext } from "./session-context-hydrator";

describe("session chat context hydration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: {
        segments: [
          {
            speaker_label: "SQLite Person",
            text: "Transcript text",
            start_ms: 4_000,
          },
        ],
        started_at: 100,
        ended_at: 200,
      },
    });
    mocks.loadMeetingChatRecords.mockResolvedValue([
      { text: "Review the rollout plan" },
    ]);
    mocks.formatMeetingChatRecordsAsMarkdown.mockReturnValue(
      "- Slack · 10:42 AM · Ada · received\n  Review the rollout plan",
    );
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
        sessionId: "session-1",
        title: "Planning",
        date: "2026-07-10T09:00:00.000Z",
        rawContent: "Raw note",
        enhancedContent: "First\n\n---\n\nSecond",
        meetingChat:
          "- Slack · 10:42 AM · Ada · received\n  Review the rollout plan",
        transcript: {
          segments: [
            {
              speaker: "SQLite Person",
              text: "Transcript text",
              startMs: 4_000,
            },
          ],
          startedAt: 100,
          endedAt: 200,
        },
        participants: [{ name: "SQLite Person", jobTitle: "Engineer" }],
        event: { name: "Weekly planning" },
      },
    );
  });

  it("returns a null transcript when Rust has no renderable transcript", async () => {
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: null,
    });

    const result = await hydrateSessionContext("session-1", "user-1");

    expect(result?.transcript).toBeNull();
  });

  it("returns null when the canonical session is unavailable", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValueOnce(null);

    await expect(
      hydrateSessionContext("session-missing", "user-1"),
    ).resolves.toBeNull();
  });
});
