import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(() => Promise.resolve([{ workspace_id: "workspace-1" }])),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
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

    expect(sessionId).toBe("workspace-1:devtools-recurring-notes-current");
    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);

    const statements = mocks.executeTransaction.mock.calls[0][0];
    const sessionInserts = statements.filter((statement) =>
      statement.sql.includes("INSERT INTO sessions"),
    );
    expect(sessionInserts).toHaveLength(4);
    expect(
      sessionInserts.every((statement) =>
        statement.params.includes("workspace-1"),
      ),
    ).toBe(true);
    expect(
      sessionInserts.every((statement) =>
        String(statement.params[0]).startsWith("workspace-1:"),
      ),
    ).toBe(true);
    const participantInserts = statements.filter((statement) =>
      statement.sql.includes("INSERT INTO session_participants"),
    );
    expect(participantInserts).toHaveLength(12);
    expect(
      participantInserts.every((statement) =>
        statement.sql.includes("session.workspace_id"),
      ),
    ).toBe(true);
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

  test("namespaces synced fixture ids by workspace", async () => {
    mocks.execute
      .mockResolvedValueOnce([{ workspace_id: "workspace-a" }])
      .mockResolvedValueOnce([{ workspace_id: "workspace-b" }]);

    await populateRecurringMeetingNotes({ userId: null });
    await populateRecurringMeetingNotes({ userId: null });

    const sessionIds = mocks.executeTransaction.mock.calls.map((call) =>
      call[0]
        .filter((statement) => statement.sql.includes("INSERT INTO sessions"))
        .map((statement) => statement.params[0]),
    );
    expect(sessionIds[0]).toHaveLength(4);
    expect(sessionIds[1]).toHaveLength(4);
    expect(new Set(sessionIds[0])).not.toEqual(new Set(sessionIds[1]));
    expect(
      sessionIds[0]?.every((id) => String(id).startsWith("workspace-a:")),
    ).toBe(true);
    expect(
      sessionIds[1]?.every((id) => String(id).startsWith("workspace-b:")),
    ).toBe(true);
  });
});
