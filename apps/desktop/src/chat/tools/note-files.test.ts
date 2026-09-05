import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  formatMeetingChatRecordsAsMarkdown: vi.fn(),
  loadActiveSessionIds: vi.fn(),
  loadMeetingChatRecords: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  loadSessionSummariesByFolder: vi.fn(),
}));

vi.mock("~/stt/meeting-chat-records", () => ({
  formatMeetingChatRecordsAsMarkdown: mocks.formatMeetingChatRecordsAsMarkdown,
  loadMeetingChatRecords: mocks.loadMeetingChatRecords,
}));

vi.mock("~/session/content-queries", () => ({
  loadActiveSessionIds: mocks.loadActiveSessionIds,
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/session/queries", () => ({
  loadSessionSummariesByFolder: mocks.loadSessionSummariesByFolder,
}));

import {
  buildSearchMeetingContentTool,
  noteFileTestInternals,
} from "./note-files";

const snapshot = {
  sessionId: "session-1",
  title: "Customer call",
  createdAt: "2026-06-02T00:00:00.000Z",
  event: { title: "Customer sync" },
  eventId: "event-1",
  rawMarkdown: "Discussed contract renewal timing.",
  enhancedNotes: [],
  transcripts: [],
  participants: [
    { humanId: "human-1", name: "Ada Lovelace", jobTitle: "Founder" },
  ],
};

describe("note file chat tools", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadActiveSessionIds.mockResolvedValue(["session-1"]);
    mocks.formatMeetingChatRecordsAsMarkdown.mockReturnValue("");
    mocks.loadMeetingChatRecords.mockResolvedValue([]);
    mocks.loadSessionContentSnapshot.mockResolvedValue(snapshot);
    mocks.loadSessionSummariesByFolder.mockResolvedValue([]);
  });

  it("extracts raw, enhanced, and transcript sections from session files", () => {
    const sections = noteFileTestInternals.buildNoteSections(
      {
        rawMarkdown: "Raw memo",
        enhancedNotes: [
          {
            id: "summary-1",
            title: "Summary",
            markdown: "Enhanced note",
            position: 1,
          },
        ],
        transcripts: [
          {
            id: "transcript-1",
            memo: "",
            words: [
              {
                text: "Hello",
                start_ms: 0,
                end_ms: 100,
                channel: 0,
              },
              {
                text: "world",
                start_ms: 100,
                end_ms: 200,
                channel: 0,
              },
            ],
          },
        ],
      } as any,
      "- Slack · Ada\n  Review the rollout plan",
    );

    expect(sections).toEqual([
      { title: "Raw note", text: "Raw memo" },
      {
        title: "Meeting chat",
        text: "- Slack · Ada\n  Review the rollout plan",
      },
      { title: "Summary", text: "Enhanced note" },
      { title: "Transcript", text: "Hello world" },
    ]);
  });

  it("searches content that appears only in captured meeting chat", async () => {
    mocks.loadMeetingChatRecords.mockResolvedValue([
      { text: "Use the canary rollout phrase" },
    ]);
    mocks.formatMeetingChatRecordsAsMarkdown.mockReturnValue(
      "- Slack · Ada\n  Use the canary rollout phrase",
    );

    const tool = buildSearchMeetingContentTool({} as any);
    const result = await (tool as any).execute({ query: "canary rollout" });

    expect(result.results[0]?.meeting_id).toBe("session-1");
    expect(
      result.results[0]?.snippets.some(
        (snippet: { section: string }) => snippet.section === "Meeting chat",
      ),
    ).toBe(true);
  });

  it("matches lexical note content and returns snippets", () => {
    const result = noteFileTestInternals.searchNote(
      {
        sessionId: "session-1",
        title: "Customer call",
        date: "2026-06-02T00:00:00.000Z",
        eventName: null,
        eventId: null,
        participantIds: [],
        participants: ["Ada Lovelace"],
        sections: [
          {
            title: "Transcript",
            text: "Ada asked about contract renewal timing and next steps.",
          },
        ],
      },
      "contract renewal",
    );

    expect(result?.sessionId).toBe("session-1");
    expect(result?.snippets[0]?.section).toBe("Transcript");
    expect(result?.snippets[0]?.text).toContain("contract renewal");
  });

  it("searches canonical SQLite note snapshots", async () => {
    const contentSearchTool = buildSearchMeetingContentTool({} as any);
    const contentSearchResult = await (contentSearchTool as any).execute({
      query: "contract renewal",
    });

    expect(contentSearchResult).toMatchObject({
      query: "contract renewal",
      scanned: 1,
      results: [expect.objectContaining({ meeting_id: "session-1" })],
    });
    expect(mocks.loadActiveSessionIds).toHaveBeenCalledOnce();
    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledWith("session-1");
  });

  it("does not scan all notes when a folder has no matching sessions", async () => {
    mocks.loadSessionSummariesByFolder.mockResolvedValue([]);
    mocks.loadActiveSessionIds.mockResolvedValue(["session-1"]);

    const tool = buildSearchMeetingContentTool({
      getFolderFilter: () => "CS 101",
    } as any);
    const result = await (tool as any).execute({ query: "midterm" });

    expect(result).toMatchObject({
      query: "midterm",
      scanned: 0,
      results: [],
    });
    expect(mocks.loadActiveSessionIds).not.toHaveBeenCalled();
    expect(mocks.loadSessionContentSnapshot).not.toHaveBeenCalled();
  });

  it("does not scan all notes when meeting ids fall outside the folder", async () => {
    mocks.loadSessionSummariesByFolder.mockResolvedValue([
      {
        id: "cs-101",
        title: "Lecture",
        created_at: "2026-08-01T00:00:00.000Z",
      },
    ]);

    const tool = buildSearchMeetingContentTool({
      getFolderFilter: () => "CS 101",
    } as any);
    const result = await (tool as any).execute({
      query: "midterm",
      meeting_ids: ["other-folder"],
    });

    expect(result).toMatchObject({
      query: "midterm",
      scanned: 0,
      results: [],
    });
    expect(mocks.loadActiveSessionIds).not.toHaveBeenCalled();
    expect(mocks.loadSessionContentSnapshot).not.toHaveBeenCalled();
  });

  it("returns metadata snippets for participant matches", () => {
    const result = noteFileTestInternals.searchNote(
      {
        sessionId: "session-1",
        title: "Customer call",
        date: "2026-06-02T00:00:00.000Z",
        eventName: null,
        eventId: null,
        participantIds: [],
        participants: ["Ada Lovelace"],
        sections: [{ title: "Raw note", text: "Follow-up needed." }],
      },
      "Ada",
    );

    expect(result?.snippets[0]).toEqual({
      section: "Participants",
      text: "Ada Lovelace",
    });
  });
});
