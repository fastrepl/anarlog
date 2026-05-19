import { describe, expect, test } from "vitest";

import { md2json } from "@hypr/editor/markdown";

import { legacySessionToMarkdownDocument } from "./legacy-file-converter";

import { createEmptyLoadedSessionData } from "~/store/tinybase/persister/session/load";

function richText(markdown: string): string {
  return JSON.stringify(md2json(markdown));
}

describe("legacySessionToMarkdownDocument", () => {
  test("converts legacy session tables into the canonical markdown document", () => {
    const data = createEmptyLoadedSessionData();
    data.sessions["session-1"] = {
      user_id: "user-1",
      created_at: "2026-05-19T00:00:00.000Z",
      folder_id: "work",
      event_json: JSON.stringify({
        tracking_id_event: "event-1",
        title: "Weekly sync",
      }),
      title: "Weekly sync",
      raw_md: richText("Raw notes.\n\n- follow up"),
    };
    data.mapping_session_participant["participant-1"] = {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    };
    data.tags["tag-1"] = {
      user_id: "user-1",
      name: "work",
    };
    data.mapping_tag_session["mapping-1"] = {
      user_id: "user-1",
      session_id: "session-1",
      tag_id: "tag-1",
    };
    data.enhanced_notes["summary-2"] = {
      user_id: "user-1",
      session_id: "session-1",
      content: richText("Second summary."),
      template_id: "summary",
      position: 2,
      title: "Later",
    };
    data.enhanced_notes["summary-1"] = {
      user_id: "user-1",
      session_id: "session-1",
      content: richText("First summary."),
      template_id: "summary",
      position: 1,
      title: "Earlier",
    };
    data.transcripts["transcript-1"] = {
      user_id: "user-1",
      created_at: "2026-05-19T00:00:00.000Z",
      session_id: "session-1",
      started_at: 20,
      ended_at: 30,
      words: JSON.stringify([
        {
          text: "Hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          speaker: "Jane",
        },
        {
          text: ".",
          start_ms: 100,
          end_ms: 120,
          channel: 0,
          speaker: "Jane",
        },
        {
          text: "Hi",
          start_ms: 130,
          end_ms: 200,
          channel: 0,
          speaker: "Sam",
        },
      ]),
      speaker_hints: "[]",
      memo_md: "Transcript memo.",
    };

    expect(legacySessionToMarkdownDocument(data, "session-1")).toEqual({
      schemaVersion: 1,
      id: "session-1",
      createdAt: "2026-05-19T00:00:00.000Z",
      title: "Weekly sync",
      folderId: "work",
      eventId: "event-1",
      event: {
        tracking_id_event: "event-1",
        title: "Weekly sync",
      },
      participants: [
        {
          legacy_human_id: "human-1",
          source: "manual",
        },
      ],
      tags: ["work"],
      notes: "Raw notes.\n\n- follow up",
      summary: "## Earlier\n\nFirst summary.\n\n## Later\n\nSecond summary.",
      transcript: "Jane: Hello.\nSam: Hi\n\nTranscript memo.",
    });
  });

  test("returns null when the session does not exist", () => {
    const data = createEmptyLoadedSessionData();

    expect(legacySessionToMarkdownDocument(data, "missing")).toBeNull();
  });

  test("preserves malformed legacy rich text as markdown text", () => {
    const data = createEmptyLoadedSessionData();
    data.sessions["session-1"] = {
      user_id: "user-1",
      created_at: "",
      folder_id: "",
      event_json: "legacy-event-id",
      title: "",
      raw_md: "plain markdown",
    };
    data.enhanced_notes["summary-1"] = {
      user_id: "user-1",
      session_id: "session-1",
      content: "{bad json",
      template_id: "",
      position: 0,
      title: "",
    };
    data.transcripts["transcript-1"] = {
      user_id: "user-1",
      created_at: "",
      session_id: "session-1",
      started_at: 0,
      ended_at: undefined,
      words: "{bad json",
      speaker_hints: "[]",
      memo_md: "",
    };

    expect(legacySessionToMarkdownDocument(data, "session-1")).toMatchObject({
      createdAt: "1970-01-01T00:00:00.000Z",
      eventId: "legacy-event-id",
      notes: "plain markdown",
      summary: "{bad json",
      transcript: "",
    });
  });
});
