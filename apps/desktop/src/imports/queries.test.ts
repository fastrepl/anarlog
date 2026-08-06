import { describe, expect, it } from "vitest";

import { buildMeetingStatements } from "./queries";

describe("meeting import statements", () => {
  it("creates every supported meeting record without overwriting sessions", () => {
    const statements = buildMeetingStatements({
      providerId: "otter",
      sourcePath: "/exports/meeting.json",
      sessionId: "meeting-import:one",
      meeting: {
        externalId: "external-one",
        title: "Weekly planning",
        startedAt: "2026-08-06T01:00:00.000Z",
        endedAt: "2026-08-06T01:30:00.000Z",
        sourceUrl: "https://otter.ai/u/one",
        noteMarkdown: "## Summary\n\nDecided to ship.",
        transcript: [
          {
            speaker: "Ada",
            text: "Let's ship.",
            startMs: 0,
            endMs: 1_000,
          },
        ],
        attendees: [{ name: "Ada", email: "ada@example.com" }],
        actionItems: ["Prepare the release"],
      },
    });

    for (const table of [
      "sessions",
      "session_documents",
      "transcripts",
      "session_participants",
      "action_items",
    ]) {
      expect(
        statements.some(({ sql }) => sql.includes(`INSERT INTO ${table}`)),
      ).toBe(true);
    }
    expect(
      statements.every(({ sql }) => !sql.includes("UPDATE sessions")),
    ).toBe(true);
    for (const statement of statements) {
      expect(statement.sql.match(/\?/gu)?.length ?? 0).toBe(
        statement.params.length,
      );
    }
  });
});
