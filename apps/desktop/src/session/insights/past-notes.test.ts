import { describe, expect, it } from "vitest";

import { buildPastSessionNotes } from "./past-notes";

describe("buildPastSessionNotes", () => {
  it("builds descending past notes from recurring and same-title sessions", () => {
    const store = makeStore({
      sessions: {
        current: {
          title: "Weekly Product Sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-06-03T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
          raw_md: "",
        },
        previous: {
          title: "Weekly Product Sync",
          created_at: "2026-05-28T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-05-28T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
          raw_md: "",
        },
        same_title: {
          title: "Weekly Product Sync",
          created_at: "2026-05-27T10:00:00.000Z",
          event_json: "",
          raw_md: "Raw note text should not feed insights.",
        },
        older: {
          title: "Older Product Sync",
          created_at: "2026-05-21T10:00:00.000Z",
          event_json: "",
          raw_md: "Reviewed onboarding follow-ups and assigned owners.",
        },
        future: {
          title: "Future Product Sync",
          created_at: "2026-06-10T10:00:00.000Z",
          event_json: "",
          raw_md: "Should not show up.",
        },
      },
      mapping_session_participant: {
        current_self: {
          session_id: "current",
          human_id: "self",
          user_id: "self",
          source: "manual",
        },
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        current_jamie: {
          session_id: "current",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        previous_alex: {
          session_id: "previous",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        previous_jamie: {
          session_id: "previous",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        same_title_alex: {
          session_id: "same_title",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        same_title_jamie: {
          session_id: "same_title",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        older_alex: {
          session_id: "older",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        older_jamie: {
          session_id: "older",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        future_alex: {
          session_id: "future",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        future_jamie: {
          session_id: "future",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
      },
      enhanced_notes: {
        previous_summary: {
          session_id: "previous",
          content:
            "Aligned on transcript panel behavior. Past notes should stay short and scannable.",
          position: 0,
        },
        same_title_summary: {
          session_id: "same_title",
          content: "Confirmed notification copy and reviewed follow-ups.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(store, "current", "self");

    expect(result.notes).toEqual([
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        participantNames: ["alex", "jamie"],
        summary: null,
        isGenerating: false,
      },
      {
        sessionId: "same_title",
        title: "Weekly Product Sync",
        dateLabel: "May 27, 2026",
        participantNames: ["alex", "jamie"],
        summary: null,
        isGenerating: false,
      },
    ]);
    expect(result.missing.map((request) => request.sessionId)).toEqual([
      "previous",
      "same_title",
    ]);
    expect(result.requests.map((request) => request.sourceText)).toEqual([
      "Aligned on transcript panel behavior. Past notes should stay short and scannable.",
      "Confirmed notification copy and reviewed follow-ups.",
    ]);
  });

  it("does not treat matching participants alone as related past notes", () => {
    const store = makeStore({
      sessions: {
        current: {
          title: "Design sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: "",
          raw_md: "",
        },
        different_topic: {
          title: "Dev sync",
          created_at: "2026-06-01T10:00:00.000Z",
          event_json: "",
          raw_md: "Discussed release branch status.",
        },
      },
      mapping_session_participant: {
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        current_jamie: {
          session_id: "current",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        different_topic_alex: {
          session_id: "different_topic",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        different_topic_jamie: {
          session_id: "different_topic",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
      },
    });

    const result = buildPastSessionNotes(store, "current", "self");

    expect(result.notes).toEqual([]);
    expect(result.missing).toEqual([]);
  });
});

function makeStore(
  tables: Record<string, Record<string, Record<string, unknown>>>,
) {
  return {
    getRow: (tableId: string, rowId: string) => tables[tableId]?.[rowId] ?? {},
    getCell: (tableId: string, rowId: string, cellId: string) =>
      tables[tableId]?.[rowId]?.[cellId],
    forEachRow: (
      tableId: string,
      callback: (rowId: string, forEachCell: unknown) => void,
    ) => {
      for (const rowId of Object.keys(tables[tableId] ?? {})) {
        callback(rowId, () => {});
      }
    },
    setRow: (tableId: string, rowId: string, row: Record<string, unknown>) => {
      tables[tableId] = {
        ...(tables[tableId] ?? {}),
        [rowId]: row,
      };
    },
  } as any;
}
