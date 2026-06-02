import { describe, expect, it } from "vitest";

import { noteFileTestInternals } from "./note-files";

describe("note file chat tools", () => {
  it("extracts raw, enhanced, and transcript sections from session files", () => {
    const sections = noteFileTestInternals.buildNoteSections({
      rawMemoMarkdown: "Raw memo",
      notes: [
        {
          title: "Summary",
          markdown: "Enhanced note",
          position: 1,
        },
      ],
      transcript: {
        transcripts: [
          {
            id: "transcript-1",
            session_id: "session-1",
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
      },
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
