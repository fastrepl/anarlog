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
