import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<number[]>) => write(),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

vi.mock("~/shared/utils", () => ({
  id: vi.fn(() => "new-job-id"),
}));

import { reconcileAttachmentTransferJobs } from "./store";

describe("attachment transfer reconciliation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([1, 1]);
  });

  it("supersedes a stale failed download before inserting its replacement", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          id: "attachment-1",
          session_id: "session-1",
          workspace_id: "workspace-1",
          sha256: "a".repeat(64),
          size_bytes: 42,
          cloud_object_key: "owner/current.anb1",
          cloud_sync_enabled: 1,
          deleted_at: null,
          local_availability: "absent",
        },
      ])
      .mockResolvedValueOnce([{ id: "stale-download-job" }]);

    await expect(reconcileAttachmentTransferJobs()).resolves.toBe(2);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("phase = 'completed'");
    expect(statements[0].sql).toContain(
      "attachment.cloud_object_key = job.object_key",
    );
    expect(statements[0].params[2]).toBe("stale-download-job");
    expect(statements[1].sql).toContain("INSERT OR IGNORE");
    expect(statements[1].params).toEqual([
      "new-job-id",
      "attachment-1",
      "session-1",
      "workspace-1",
      "download",
      "a".repeat(64),
      42,
      "owner/current.anb1",
    ]);
  });

  it("retires obsolete failed downloads even when no replacement is needed", async () => {
    mocks.execute
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ id: "obsolete-download-job" }]);
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await expect(reconcileAttachmentTransferJobs()).resolves.toBe(1);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(1);
    expect(statements[0].params[2]).toBe("obsolete-download-job");
  });
});
