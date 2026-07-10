import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadActiveSessionIds: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
}));

vi.mock("~/session/content-queries", () => ({
  loadActiveSessionIds: mocks.loadActiveSessionIds,
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

import {
  buildGrepNotesTool,
  buildReadCurrentNoteTool,
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
    mocks.loadSessionContentSnapshot.mockResolvedValue(snapshot);
  });

  it("extracts raw, enhanced, and transcript sections from session files", () => {
    const sections = noteFileTestInternals.buildNoteSections({
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
    } as any);

    expect(sections).toEqual([
      { title: "Raw note", text: "Raw memo" },
      { title: "Summary", text: "Enhanced note" },
      { title: "Transcript", text: "Hello world" },
    ]);
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

  it("reads and searches canonical SQLite note snapshots", async () => {
    const readTool = buildReadCurrentNoteTool({
      getSessionId: () => "session-1",
    } as any);
    const readResult = await (readTool as any).execute({});

    expect(readResult).toMatchObject({
      status: "ok",
      sessionId: "session-1",
      title: "Customer call",
      participants: ["Ada Lovelace"],
    });
    expect(readResult.contextText).toContain("contract renewal timing");

    const grepTool = buildGrepNotesTool({} as any);
    const grepResult = await (grepTool as any).execute({
      query: "contract renewal",
    });

    expect(grepResult).toMatchObject({
      query: "contract renewal",
      scanned: 1,
      results: [expect.objectContaining({ sessionId: "session-1" })],
    });
    expect(mocks.loadActiveSessionIds).toHaveBeenCalledOnce();
    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledWith("session-1");
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
