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

  it("handles quoted multiline CSV cells", () => {
    expect(
      parseCsvRows('title,notes\n"Planning","Line one\nLine two"'),
    ).toEqual([
      ["title", "notes"],
      ["Planning", "Line one\nLine two"],
    ]);
  });
});
