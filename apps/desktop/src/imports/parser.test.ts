import { describe, expect, it } from "vitest";

import { parseCsvRows, parseMeetingExport } from "./parser";

describe("meeting export parser", () => {
  it("parses provider JSON with structured meeting data", () => {
    const [meeting] = parseMeetingExport({
      path: "/tmp/meetings.json",
      name: "meetings.json",
      content: JSON.stringify({
        meetings: [
          {
            id: "meeting-1",
            title: "Weekly planning",
            started_at: "2026-08-01T10:00:00Z",
            summary: "We agreed to ship.",
            transcript: [{ speaker: "Alex", text: "Let's ship it." }],
            participants: [{ name: "Alex", email: "alex@example.com" }],
            action_items: [{ text: "Prepare the release" }],
          },
        ],
      }),
    });

    expect(meeting).toMatchObject({
      externalId: "meeting-1",
      title: "Weekly planning",
      startedAt: "2026-08-01T10:00:00.000Z",
      actionItems: ["Prepare the release"],
      attendees: [{ name: "Alex", email: "alex@example.com" }],
    });
    expect(meeting?.noteMarkdown).toContain("We agreed to ship.");
    expect(meeting?.transcript[0]).toMatchObject({
      speaker: "Alex",
      text: "Let's ship it.",
    });
  });

  it("preserves Anarlog package transcripts and summary evidence metadata", () => {
    const [meeting] = parseMeetingExport({
      path: "/tmp/Weekly.anlgmeeting",
      name: "Weekly.anlgmeeting",
      content: JSON.stringify({
        format: "anarlog.meeting",
        version: 1,
        meeting: {
          title: "Weekly",
          created_at: "2026-10-01T09:00:00.000Z",
          note: "Notes",
          speaker_context: { Alex: "Product" },
          summaries: [
            {
              title: "Minutes",
              body: "Decision",
              body_format: "markdown",
              generation_metadata_json: "{}",
            },
          ],
        },
        transcripts: [
          {
            id: "original-transcript",
            source: "microphone",
            provider: "local-whisper",
            model: "small",
            language: "en",
            started_at_ms: 0,
            ended_at_ms: 1200,
            memo: "",
            words_json: JSON.stringify([
              { id: "word-1", text: "Hello", start_ms: 0, end_ms: 1200 },
            ]),
            speaker_hints_json: "[]",
            metadata_json: JSON.stringify({
              transcript_edits: {
                "original-transcript:stable-segment": {
                  text: "Hello there",
                },
              },
            }),
          },
        ],
        audio: null,
      }),
    });

    expect(meeting).toMatchObject({
      title: "Weekly",
      speakerContext: { Alex: "Product" },
      packageTranscripts: [
        {
          id: "original-transcript",
          provider: "local-whisper",
          metadata_json: expect.stringContaining("transcript_edits"),
        },
      ],
      summaryDocuments: [{ title: "Minutes", body: "Decision" }],
    });
  });

  it("rejects an invalid Anarlog transcript instead of silently dropping it", () => {
    expect(() =>
      parseMeetingExport({
        path: "/tmp/bad.anlgmeeting",
        name: "bad.anlgmeeting",
        content: JSON.stringify({
          format: "anarlog.meeting",
          version: 1,
          meeting: {
            title: "Bad",
            created_at: "2026-10-01T09:00:00.000Z",
            summaries: [],
          },
          transcripts: [{ id: "t", words_json: "{}" }],
          audio: null,
        }),
      }),
    ).toThrow("invalid transcript");
  });

  it("parses captions and preserves speaker labels", () => {
    const [meeting] = parseMeetingExport({
      path: "/tmp/meeting.vtt",
      name: "meeting.vtt",
      content:
        "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\n<v Priya>Hello team\n\n00:00:03.000 --> 00:00:05.000\nSam: Hi Priya",
    });

    expect(meeting?.transcript).toEqual([
      { speaker: "Priya", text: "Hello team", startMs: 1_000, endMs: 3_000 },
      { speaker: "Sam", text: "Hi Priya", startMs: 3_000, endMs: 5_000 },
    ]);
  });

  it("parses Granola MCP meeting fields", () => {
    const [meeting] = parseMeetingExport({
      path: "mcp://granola/meeting-1.json",
      name: "meeting-1.json",
      content: JSON.stringify({
        document_id: "meeting-1",
        meetingTitle: "Customer handoff",
        meeting_date: "2026-08-08T04:30:00Z",
        enhanced_notes: "## Decisions\n\nMove forward with the migration.",
        granola_url: "https://app.granola.ai/d/meeting-1",
      }),
    });

    expect(meeting).toMatchObject({
      externalId: "meeting-1",
      title: "Customer handoff",
      startedAt: "2026-08-08T04:30:00.000Z",
      sourceUrl: "https://app.granola.ai/d/meeting-1",
    });
    expect(meeting?.noteMarkdown).toContain("Move forward with the migration");
  });

  it("parses Pocket MCP recording fields", () => {
    const [meeting] = parseMeetingExport({
      path: "mcp://pocket/rec_123.json",
      name: "rec_123.json",
      content: JSON.stringify({
        recordingId: "rec_123",
        recordingTitle: "Weekly Sync",
        recordingDate: "2026-03-25T15:04:05Z",
        transcriptSegments: [
          {
            text: "Let's review the launch plan.",
            start: 0.62,
            end: 4.88,
            speaker: "Alex",
          },
        ],
        summary: { text: "Finalize QA by Friday." },
      }),
    });

    expect(meeting).toMatchObject({
      externalId: "rec_123",
      title: "Weekly Sync",
      startedAt: "2026-03-25T15:04:05.000Z",
    });
    expect(meeting?.noteMarkdown).toContain("Finalize QA by Friday.");
    expect(meeting?.transcript[0]).toMatchObject({
      speaker: "Alex",
      text: "Let's review the launch plan.",
      startMs: 620,
      endMs: 4_880,
    });
  });

  it("imports each meeting from a titled collection export", () => {
    const meetings = parseMeetingExport({
      path: "/tmp/export.json",
      name: "export.json",
      content: JSON.stringify({
        title: "March export",
        topic: "search",
        meetings: [
          { id: "meeting-1", title: "Weekly planning" },
          { id: "meeting-2", title: "Customer call" },
        ],
      }),
    });

    expect(meetings.map((meeting) => meeting.title)).toEqual([
      "Weekly planning",
      "Customer call",
    ]);
  });

  it("normalizes call-oriented MCP meeting fields", () => {
    const [meeting] = parseMeetingExport({
      path: "mcp://jiminny/call-1.json",
      name: "call-1.json",
      content: JSON.stringify({
        call_id: "call-1",
        call_title: "Customer discovery",
        summary: ["Pricing is the blocker", "Follow up next week"],
        recording_url: "https://example.com/calls/call-1",
      }),
    });

    expect(meeting).toMatchObject({
      externalId: "call-1",
      title: "Customer discovery",
      sourceUrl: "https://example.com/calls/call-1",
    });
    expect(meeting?.noteMarkdown).toContain("Pricing is the blocker");
    expect(meeting?.noteMarkdown).toContain("Follow up next week");
  });

  it("handles quoted multiline CSV cells", () => {
    expect(
      parseCsvRows('title,notes\n"Planning","Line one\nLine two"'),
    ).toEqual([
      ["title", "notes"],
      ["Planning", "Line one\nLine two"],
    ]);
  });
});
