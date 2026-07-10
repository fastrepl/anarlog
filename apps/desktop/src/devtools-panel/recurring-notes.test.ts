import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import { populateRecurringMeetingNotes } from "./recurring-notes";

describe("populateRecurringMeetingNotes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test("seeds recurring sessions and cached facts in one SQLite transaction", async () => {
    const sessionId = await populateRecurringMeetingNotes({
      userId: "user-1",
      now: new Date("2026-06-03T10:00:00.000Z"),
    });

    expect(sessionId).toBe("devtools-recurring-notes-current");
    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(
      statements.filter((statement) =>
        statement.sql.includes("INSERT INTO sessions"),
      ),
    ).toHaveLength(4);
    expect(
      statements.filter((statement) =>
        statement.sql.includes("INSERT INTO session_participants"),
      ),
    ).toHaveLength(12);
    const keyFactInserts = statements.filter(
      (statement) =>
        statement.sql.includes("INSERT INTO session_documents") &&
        statement.sql.includes("'key_facts'"),
    );
    expect(keyFactInserts).toHaveLength(3);
    expect(keyFactInserts[0]?.params).toContain(
      "Transcript controls shipped with a condensed panel layout.\nAlex owns the launch checklist and analytics confirmation.\nMaya wants another empty-state pass after beta feedback.",
    );
  });
});
