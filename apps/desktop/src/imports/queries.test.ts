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

  it("preserves package transcript rows and remaps summary evidence links", () => {
    const statements = buildMeetingStatements({
      providerId: "anarlog",
      sourcePath: "/exports/meeting.anlgmeeting",
      sessionId: "meeting-import:two",
      meeting: {
        externalId: "source-id",
        title: "Weekly",
        startedAt: "2026-08-06T01:00:00.000Z",
        endedAt: "",
        sourceUrl: "",
        noteMarkdown: "Notes",
        transcript: [],
        packageTranscripts: [
          {
            id: "old-transcript",
            source: "microphone",
            provider: "whisper.cpp",
            model: "small",
            language: "vi",
            started_at_ms: 0,
            ended_at_ms: 1_000,
            memo: "",
            words_json: JSON.stringify([
              { id: "old-word", text: "Xin chào", start_ms: 0, end_ms: 1_000 },
            ]),
            speaker_hints_json: "[]",
            metadata_json: JSON.stringify({
              transcript_edits: {
                "old-transcript:stable-segment": {
                  text: "Xin chào mọi người",
                  revisions: [{ text: "Xin chào", edited_at: "2026-08-06" }],
                },
              },
              custom_metadata: "preserved",
            }),
          },
        ],
        summaryDocuments: [
          {
            title: "Minutes",
            body: "[source](anarlog-evidence://old-transcript%3Aold-word)",
            bodyFormat: "markdown",
            generationMetadataJson: "{}",
          },
        ],
        meetingAudio: {
          path: "/tmp/audio.m4a",
          filename: "audio.m4a",
          contentType: "audio/mp4",
          sizeBytes: 128,
          sha256: "a".repeat(64),
        },
        attendees: [],
        actionItems: [],
      },
    });

    const transcript = statements.find(({ sql }) =>
      sql.includes("INSERT INTO transcripts"),
    );
    const summary = statements.find(({ sql }) => sql.includes("'summary'"));
    expect(transcript?.sql).toContain("audio_attachment_id");
    expect(transcript?.params).toContain("session-audio:meeting-import:two");
    expect(JSON.parse(String(transcript?.params[9]))).toEqual([
      { id: "old-word", text: "Xin chào", start_ms: 0, end_ms: 1_000 },
    ]);
    expect(JSON.parse(String(transcript?.params[11]))).toEqual({
      transcript_edits: {
        "meeting-import:two:transcript:0:stable-segment": {
          text: "Xin chào mọi người",
          revisions: [{ text: "Xin chào", edited_at: "2026-08-06" }],
        },
      },
      custom_metadata: "preserved",
      importedFrom: "anarlog",
      sourcePath: "/exports/meeting.anlgmeeting",
      sourceUrl: "",
      externalId: "source-id",
    });
    expect(summary?.params).toContain(
      "[source](anarlog-evidence://meeting-import%3Atwo%3Atranscript%3A0%3Aold-word)",
    );
    for (const statement of statements) {
      expect(statement.sql.match(/\?/gu)?.length ?? 0).toBe(
        statement.params.length,
      );
    }
  });
});
