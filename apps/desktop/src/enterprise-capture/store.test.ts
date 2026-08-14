import { beforeEach, describe, expect, it, vi } from "vitest";

import { recordRejectedDelivery } from "./store";

const mocks = vi.hoisted(() => ({ executeTransaction: vi.fn() }));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: vi.fn() },
}));

describe("enterprise capture store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue(undefined);
  });

  it("advances rejected deliveries without enqueueing completion", async () => {
    await recordRejectedDelivery({
      serverUrl: "https://capture.example.test",
      workspaceId: "workspace-1",
      consumerId: "device-1",
      item: {
        cursor: 5,
        jobId: "job-1",
        revision: 2,
        finalized: true,
        contentHash: "a".repeat(64),
        acknowledged: false,
        createdAt: "2026-08-14T08:00:00Z",
        envelope: {
          schema_version: 1,
          source_id: "job-1",
          revision: 2,
          finalized: true,
          workspace_id: "workspace-1",
          session: { id: "session-1", status: "completed" },
        },
      },
    });

    const statements = mocks.executeTransaction.mock.calls[0]?.[0];
    expect(statements).toHaveLength(2);
    expect(
      statements.some(({ sql }: { sql: string }) =>
        sql.includes("enterprise_session_completion_outbox"),
      ),
    ).toBe(false);
  });
});
