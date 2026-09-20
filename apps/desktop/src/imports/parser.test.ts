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

  it("parses Fireflies timestamped speaker turns from a single text blob", () => {
    const [meeting] = parseMeetingExport({
      path: "/tmp/fireflies.txt",
      name: "fireflies.txt",
      content:
        "01M2QPG5DND30EDFVQWAED6CWJ 2026-09-18T10:00:00.000Z link Alexis Murat, Camille Vingere " +
        "[00:02 - 00:03] Alexis Murat: Le classique. " +
        "[00:04 - 00:07] Alexis Murat: Hello, hello, comment ça va? " +
        "[00:07 - 00:08] Camille Vingere: Ça va, et toi?",
    });

    expect(meeting?.transcript).toEqual([
      {
        speaker: "Alexis Murat",
        text: "Le classique.",
        startMs: 2_000,
        endMs: 3_000,
      },
      {
        speaker: "Alexis Murat",
        text: "Hello, hello, comment ça va?",
        startMs: 4_000,
        endMs: 7_000,
      },
      {
        speaker: "Camille Vingere",
        text: "Ça va, et toi?",
        startMs: 7_000,
        endMs: 8_000,
      },
    ]);
  });

  it("parses the record shape produced by the Fireflies export folder importer", () => {
    const [meeting] = parseMeetingExport({
      path: "/tmp/ID1.json",
      name: "ID1.json",
      content: JSON.stringify({
        id: "01M07W3NXYYBB0H2H9NCGQ2KX3",
        title: "Meet – Weekly CEOs Avicenne <> Titan",
        started_at: "2026-08-17T12:45:50.640Z",
        ended_at: "2026-08-17T13:30:33.552Z",
        url: "https://app.fireflies.ai/view/01M07W3NXYYBB0H2H9NCGQ2KX3",
        attendees: [],
        transcript: [
          {
            speaker: "Marwane Lairi",
            text: "Selam, les amis, selam!",
            start: 474.691,
            end: 481.611,
          },
        ],
      }),
    });

    expect(meeting).toMatchObject({
      externalId: "01M07W3NXYYBB0H2H9NCGQ2KX3",
      title: "Meet – Weekly CEOs Avicenne <> Titan",
      startedAt: "2026-08-17T12:45:50.640Z",
      sourceUrl: "https://app.fireflies.ai/view/01M07W3NXYYBB0H2H9NCGQ2KX3",
    });
    expect(meeting?.transcript[0]).toMatchObject({
      speaker: "Marwane Lairi",
      text: "Selam, les amis, selam!",
      startMs: 474_691,
      endMs: 481_611,
    });
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
