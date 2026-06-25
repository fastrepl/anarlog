import { describe, expect, test } from "vitest";

import { populateRecurringMeetingNotes } from "./recurring-notes";

import { buildPastSessionNotes } from "~/session/insights/past-notes";
import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";
import type { Store } from "~/store/tinybase/store/main";

describe("populateRecurringMeetingNotes", () => {
  test("seeds a recurring session with cached past note facts", () => {
    const store = createTestMainStore() as Store;
    const sessionId = populateRecurringMeetingNotes({
      store,
      userId: "user-1",
      now: new Date("2026-06-03T10:00:00.000Z"),
    });

    const result = buildPastSessionNotes(store, sessionId, "user-1");

    expect(sessionId).toBe("devtools-recurring-notes-current");
    expect(result.missing).toHaveLength(0);
    expect(result.notes.map((note) => note.sessionId)).toEqual([
      "devtools-recurring-notes-week-1",
      "devtools-recurring-notes-week-2",
      "devtools-recurring-notes-week-3",
    ]);
    expect(result.notes[0]?.summary).toContain(
      "Transcript controls shipped with a condensed panel layout.",
    );
    expect(
      store
        .getRowIds("mapping_session_participant")
        .filter((rowId) => rowId.startsWith(`${sessionId}:`)),
    ).toHaveLength(3);
  });
});
