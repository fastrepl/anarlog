import { describe, expect, it } from "vitest";

import {
  buildPastSessionNotes,
  buildSessionKeyFactsStatements,
  type PastSessionNotesData,
} from "./past-notes";

describe("buildPastSessionNotes", () => {
  it("builds descending past notes from recurring and same-title sessions", () => {
    const data = makeData({
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

    const result = buildPastSessionNotes(data, "current", "self");

    expect(result.notes).toEqual([
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        occurredAt: "2026-05-28T10:00:00.000Z",
        participantNames: ["alex", "jamie"],
        sourceSummary:
          "Aligned on transcript panel behavior. Past notes should stay short and scannable.",
        relationship: "same_series",
        summary: null,
        isGenerating: false,
      },
      {
        sessionId: "same_title",
        title: "Weekly Product Sync",
        dateLabel: "May 27, 2026",
        occurredAt: "2026-05-27T10:00:00.000Z",
        participantNames: ["alex", "jamie"],
        sourceSummary: "Confirmed notification copy and reviewed follow-ups.",
        relationship: "matching_title",
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

  it("prioritizes recurring-series history over newer title matches", () => {
    const participant = {
      human_id: "alex",
      user_id: "self",
      source: "auto",
    };
    const data = makeData({
      sessions: {
        current: {
          title: "Weekly Product Sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-06-03T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
        },
        recurring: {
          title: "Weekly Product Sync",
          created_at: "2026-05-20T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-05-20T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
        },
        title_match: {
          title: "Weekly Product Sync",
          created_at: "2026-05-30T10:00:00.000Z",
          event_json: "",
        },
      },
      mapping_session_participant: {
        current_alex: { ...participant, session_id: "current" },
        recurring_alex: { ...participant, session_id: "recurring" },
        title_match_alex: { ...participant, session_id: "title_match" },
      },
      enhanced_notes: {
        recurring_summary: {
          session_id: "recurring",
          content: "Recurring context.",
          position: 0,
        },
        title_summary: {
          session_id: "title_match",
          content: "Title-match context.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(data, "current", "self");

    expect(
      result.notes.map((note) => [note.sessionId, note.relationship]),
    ).toEqual([
      ["recurring", "same_series"],
      ["title_match", "matching_title"],
    ]);
  });

  it("uses shared participants when the meeting is not recurring or same-title", () => {
    const data = makeData({
      sessions: {
        current: {
          title: "Untitled",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-06-03T20:00:00.000Z",
          }),
          raw_md: "",
        },
        coffee_chat: {
          title: "Coffee with Yujong",
          created_at: "2026-05-20T10:00:00.000Z",
          event_json: "",
          raw_md: "",
        },
        other_person: {
          title: "Hiring loop",
          created_at: "2026-05-22T10:00:00.000Z",
          event_json: "",
          raw_md: "",
        },
      },
      mapping_session_participant: {
        current_yujong: {
          session_id: "current",
          human_id: "yujong",
          user_id: "self",
          source: "calendar",
          name: "Yujong Lee",
        },
        coffee_yujong: {
          session_id: "coffee_chat",
          human_id: "yujong",
          user_id: "self",
          source: "calendar",
          name: "Yujong Lee",
        },
        other_sam: {
          session_id: "other_person",
          human_id: "sam",
          user_id: "self",
          source: "calendar",
          name: "Sam",
        },
      },
      enhanced_notes: {
        coffee_summary: {
          session_id: "coffee_chat",
          content: "Yujong wants a tighter launch checklist before Friday.",
          position: 0,
        },
        other_summary: {
          session_id: "other_person",
          content: "Sam will schedule the next interview.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(data, "current", "self");

    expect(
      result.notes.map((note) => [note.sessionId, note.relationship]),
    ).toEqual([["coffee_chat", "shared_participants"]]);
    expect(result.notes[0]?.participantNames).toEqual(["Yujong Lee"]);
  });

  it("ranks series and title matches ahead of other meetings with the same people", () => {
    const participant = {
      human_id: "yujong",
      user_id: "self",
      source: "auto",
      name: "Yujong Lee",
    };
    const data = makeData({
      sessions: {
        current: {
          title: "Founders sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-06-03T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
        },
        series: {
          title: "Founders sync",
          created_at: "2026-05-20T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-05-20T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
        },
        titled: {
          title: "Founders sync",
          created_at: "2026-05-27T10:00:00.000Z",
          event_json: "",
        },
        other_meeting: {
          title: "Coffee chat",
          created_at: "2026-05-30T10:00:00.000Z",
          event_json: "",
        },
      },
      mapping_session_participant: {
        current_yujong: { ...participant, session_id: "current" },
        series_yujong: { ...participant, session_id: "series" },
        titled_yujong: { ...participant, session_id: "titled" },
        other_yujong: { ...participant, session_id: "other_meeting" },
      },
      enhanced_notes: {
        series_summary: {
          session_id: "series",
          content: "Series context.",
          position: 0,
        },
        titled_summary: {
          session_id: "titled",
          content: "Title context.",
          position: 0,
        },
        other_summary: {
          session_id: "other_meeting",
          content: "Coffee context.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(data, "current", "self");

    expect(
      result.notes.map((note) => [note.sessionId, note.relationship]),
    ).toEqual([
      ["series", "same_series"],
      ["titled", "matching_title"],
      ["other_meeting", "shared_participants"],
    ]);
  });
});

describe("past note relationship indexing", () => {
  function session(_id: string, createdAt: string) {
    return {
      title: "Weekly Product Sync",
      created_at: createdAt,
      event_json: "",
      raw_md: "",
    };
  }

  it("keeps duplicate participant rows and names deduplicated", () => {
    const data = makeData({
      sessions: {
        current: session("current", "2026-06-03T10:00:00.000Z"),
        previous: session("previous", "2026-05-28T10:00:00.000Z"),
      },
      mapping_session_participant: {
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
          name: "Alex",
        },
        current_alex_again: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "manual",
          name: "Alex Kim",
        },
        previous_alex: {
          session_id: "previous",
          human_id: "alex",
          user_id: "self",
          source: "auto",
          name: "",
        },
      },
      enhanced_notes: {
        previous_summary: {
          session_id: "previous",
          content: "Alex committed to send pricing by Friday.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(data, "current", "self");

    expect(result.notes).toHaveLength(1);
    // The last non-empty name for a human wins, exactly once.
    expect(result.notes[0]?.participantNames).toEqual(["Alex Kim"]);
  });

  it("skips candidates with no participants or notes without failing", () => {
    const data = makeData({
      sessions: {
        current: session("current", "2026-06-03T10:00:00.000Z"),
        no_relations: session("no_relations", "2026-05-28T10:00:00.000Z"),
      },
      enhanced_notes: {
        unrelated: {
          session_id: "no_relations",
          content: "Same title but no participant evidence.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(data, "current", "self");

    expect(result.notes).toEqual([]);
    expect(result.requests).toEqual([]);
  });

  it("keeps insertion order for recency ties", () => {
    const data = makeData({
      sessions: {
        current: session("current", "2026-06-03T10:00:00.000Z"),
        tie_a: session("tie_a", "2026-05-28T10:00:00.000Z"),
        tie_b: session("tie_b", "2026-05-28T10:00:00.000Z"),
      },
      mapping_session_participant: {
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        tie_a_alex: {
          session_id: "tie_a",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        tie_b_alex: {
          session_id: "tie_b",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
      },
      enhanced_notes: {
        a: { session_id: "tie_a", content: "Summary A", position: 0 },
        b: { session_id: "tie_b", content: "Summary B", position: 0 },
      },
    });

    const result = buildPastSessionNotes(data, "current", "self");

    expect(result.notes.map((note) => note.sessionId)).toEqual([
      "tie_a",
      "tie_b",
    ]);
  });

  it("visits each participant and note row a bounded number of times", () => {
    const sessions: Record<string, ReturnType<typeof session>> = {
      current: session("current", "2026-06-03T10:00:00.000Z"),
    };
    const participants: Record<string, Record<string, unknown>> = {
      current_p: {
        session_id: "current",
        human_id: "human-0",
        user_id: "self",
        source: "auto",
        name: "Human 0",
      },
    };
    const notes: Record<string, Record<string, unknown>> = {};
    for (let index = 0; index < 200; index += 1) {
      const id = `past-${index}`;
      sessions[id] = session(
        id,
        `2026-05-${String((index % 27) + 1).padStart(2, "0")}T10:00:00.000Z`,
      );
      participants[`${id}_p`] = {
        session_id: id,
        human_id: `human-${index % 20}`,
        user_id: "self",
        source: "auto",
        name: `Human ${index % 20}`,
      };
      notes[`${id}_n`] = {
        session_id: id,
        content: `Summary ${index}`,
        position: 0,
      };
    }

    const data = makeData({
      sessions,
      mapping_session_participant: participants,
      enhanced_notes: notes,
    });
    let participantVisits = 0;
    let noteVisits = 0;
    const countingRows = <T>(rows: T[], count: () => void): T[] =>
      new Proxy(rows, {
        get(target, property, receiver) {
          if (typeof property === "string" && /^\d+$/.test(property)) {
            count();
          }
          return Reflect.get(target, property, receiver);
        },
      });
    const counted: PastSessionNotesData = {
      ...data,
      participants: countingRows(data.participants, () => {
        participantVisits += 1;
      }),
      enhancedNotes: countingRows(data.enhancedNotes, () => {
        noteVisits += 1;
      }),
    };

    const result = buildPastSessionNotes(counted, "current", "self");

    expect(result.notes).toHaveLength(8);
    // One indexing pass (plus small constant-factor iteration overhead)
    // instead of a rescan per candidate session.
    expect(participantVisits).toBeLessThanOrEqual(data.participants.length * 4);
    expect(noteVisits).toBeLessThanOrEqual(data.enhancedNotes.length * 4);
  });
});

describe("buildSessionKeyFactsStatements", () => {
  it("copies workspace ownership from the parent session", () => {
    const statements = buildSessionKeyFactsStatements(
      [
        {
          sessionId: "session-1",
          userId: "user-1",
          content: "One fact",
          sourceHash: "hash-1",
        },
      ],
      "2026-07-13T00:00:00.000Z",
    );

    expect(statements[1]?.sql).toContain("session.workspace_id");
    expect(statements[1]?.sql).toContain("FROM sessions AS session");
    expect(statements[1]?.params).toContain("session-1");
  });
});

function makeData(
  tables: Record<string, Record<string, Record<string, unknown>>>,
): PastSessionNotesData {
  return {
    sessions: Object.fromEntries(
      Object.entries(tables.sessions ?? {}).map(([id, row]) => [
        id,
        {
          id,
          user_id: String(row.user_id ?? "self"),
          title: String(row.title ?? ""),
          created_at: String(row.created_at ?? ""),
          event_json: String(row.event_json ?? ""),
        },
      ]),
    ),
    participants: Object.values(tables.mapping_session_participant ?? {}).map(
      (row) => ({
        session_id: String(row.session_id ?? ""),
        human_id: String(row.human_id ?? ""),
        user_id: String(row.user_id ?? ""),
        source: String(row.source ?? ""),
        name: String(row.name ?? row.human_id ?? ""),
      }),
    ),
    enhancedNotes: Object.values(tables.enhanced_notes ?? {}).map((row) => ({
      session_id: String(row.session_id ?? ""),
      content: String(row.content ?? ""),
      position: Number(row.position ?? 0),
    })),
    keyFacts: Object.fromEntries(
      Object.values(tables.session_key_facts ?? {}).map((row) => [
        String(row.session_id ?? ""),
        {
          session_id: String(row.session_id ?? ""),
          user_id: String(row.user_id ?? ""),
          created_at: String(row.created_at ?? ""),
          updated_at: String(row.updated_at ?? ""),
          content: String(row.content ?? ""),
          source_hash: String(row.source_hash ?? ""),
        },
      ]),
    ),
  };
}
