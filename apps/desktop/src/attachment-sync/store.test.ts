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

import {
  claimNextAttachmentTransferJob,
  completeDelete,
  completeUpload,
  deferAttachmentTransferDeleteForPreservation,
  failAttachmentTransferJob,
  markPhase,
  prepareAttachmentTransferDelete,
  reconcileAttachmentTransferJobs,
  recoverInterruptedAttachmentTransfers,
  resetProcessLocalAttachmentTransferAttempts,
  retryAttachmentTransferJob,
  type AttachmentTransferJob,
} from "./store";

const job: AttachmentTransferJob = {
  id: "job-1",
  attachmentId: "attachment-1",
  sessionId: "session-1",
  workspaceId: "workspace-1",
  direction: "upload",
  expectedSha256: "a".repeat(64),
  expectedSizeBytes: 42,
  ciphertextSha256: "",
  ciphertextSizeBytes: 0,
  remoteObjectId: "",
  objectKey: "",
  cacheId: "cache-1",
  phase: "preparing",
  attemptCount: 7,
  cloudSyncEnabled: true,
  currentObjectKey: "",
  attachmentDeleted: false,
  localAvailability: "present",
  attachmentVersionMatches: true,
};

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
      .mockResolvedValueOnce([{ id: "stale-download-job", attempt_count: 4 }]);

    await expect(reconcileAttachmentTransferJobs()).resolves.toBe(2);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("phase = 'completed'");
    expect(statements[0].sql).toContain(
      "attachment.cloud_object_key = job.object_key",
    );
    expect(statements[0].params[2]).toBe("stale-download-job");
    expect(statements[0].params[3]).toBe(4);
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
      .mockResolvedValueOnce([
        { id: "obsolete-download-job", attempt_count: 2 },
      ]);
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await expect(reconcileAttachmentTransferJobs()).resolves.toBe(1);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(1);
    expect(statements[0].params[2]).toBe("obsolete-download-job");
  });

  it("downloads a disabled cloud-only attachment before deleting its backup", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          id: "attachment-1",
          session_id: "session-1",
          workspace_id: "workspace-1",
          sha256: "a".repeat(64),
          size_bytes: 42,
          cloud_object_key: "owner/current.anb1",
          cloud_sync_enabled: 0,
          deleted_at: null,
          local_availability: "absent",
        },
      ])
      .mockResolvedValueOnce([]);
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await reconcileAttachmentTransferJobs();

    const [statement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(statement.params[4]).toBe("download");
    expect(statement.params[7]).toBe("owner/current.anb1");
  });

  it("invalidates active process-local attempts immediately after startup", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([3]);

    await expect(resetProcessLocalAttachmentTransferAttempts()).resolves.toBe(
      3,
    );

    const [statement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(statement.sql).toContain("attempt_count = attempt_count + 1");
    expect(statement.sql).toContain("cache_id = ''");
    expect(statement.sql).toContain("WHERE phase IN");
    expect(statement.sql).not.toContain("updated_at <");
  });

  it("keeps recurring stale recovery fenced and time-bounded", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await recoverInterruptedAttachmentTransfers(
      [{ id: "active-job", attemptCount: 9 }],
      "2026-07-18T00:00:00.000Z",
    );

    const [statement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(statement.sql).toContain("attempt_count = attempt_count + 1");
    expect(statement.sql).toContain("updated_at < ?");
    expect(statement.sql).toContain("AND NOT ((id = ? AND attempt_count = ?))");
    expect(statement.params.slice(-3)).toEqual([
      "2026-07-18T00:00:00.000Z",
      "active-job",
      9,
    ]);
  });

  it("preflights current delete intent and requires a local preservation copy", async () => {
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    mocks.executeTransaction.mockResolvedValueOnce([1, 0]);

    await expect(prepareAttachmentTransferDelete(deleteJob)).resolves.toBe(
      true,
    );

    const [prepared, superseded] = mocks.executeTransaction.mock.calls[0]![0];
    expect(prepared.sql).toContain("attachment.cloud_sync_enabled = 0");
    expect(prepared.sql).toContain("local.availability, 'absent') = 'present'");
    expect(prepared.sql).toContain("attempt_count = ?");
    expect(superseded.sql).toContain("AND NOT");
  });

  it("does not preflight a delete whose intent changed", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([0, 1]);

    await expect(
      prepareAttachmentTransferDelete({
        ...job,
        direction: "delete",
        objectKey: "owner/object.anb1",
      }),
    ).resolves.toBe(false);
  });

  it("keeps an old-object delete valid after the attachment is replaced", async () => {
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-version.anb1",
    };
    mocks.executeTransaction.mockResolvedValueOnce([1, 0]);

    await expect(prepareAttachmentTransferDelete(deleteJob)).resolves.toBe(
      true,
    );

    const [prepared] = mocks.executeTransaction.mock.calls[0]![0];
    expect(prepared.sql).toContain("attachment.sha256 <> ?");
    expect(prepared.sql).toContain("attachment.size_bytes <> ?");
    expect(prepared.sql).toContain("attachment.cloud_object_key <> ?");
    expect(prepared.params.slice(-10)).toEqual([
      job.attachmentId,
      job.sessionId,
      job.workspaceId,
      job.attachmentId,
      job.sessionId,
      job.workspaceId,
      deleteJob.objectKey,
      deleteJob.objectKey,
      job.expectedSha256,
      job.expectedSizeBytes,
    ]);
  });

  it("rejects stale delete completion", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([0, 0, 0]);

    await expect(
      completeDelete({
        ...job,
        direction: "delete",
        objectKey: "owner/object.anb1",
      }),
    ).rejects.toThrow("Attachment transfer is no longer active");

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements[0].sql).toContain("job.phase = 'finalizing'");
    expect(statements[1].sql).toContain("job.phase = 'finalizing'");
    expect(statements[2].sql).toContain("phase = 'finalizing'");
  });

  it("atomically queues an exact preservation download before retiring a delete", async () => {
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    mocks.executeTransaction.mockResolvedValueOnce([1, 1, 1]);

    await expect(
      deferAttachmentTransferDeleteForPreservation(deleteJob),
    ).resolves.toBeUndefined();

    const [queue, markAbsent, complete] =
      mocks.executeTransaction.mock.calls[0]![0];
    expect(queue.sql).toContain("'download'");
    expect(queue.sql).toContain("attachment.sha256 = job.expected_sha256");
    expect(queue.sql).toContain("attachment.cloud_object_key = job.object_key");
    expect(queue.params).toEqual([
      "new-job-id",
      deleteJob.id,
      deleteJob.attemptCount,
      deleteJob.attachmentId,
      deleteJob.sessionId,
      deleteJob.workspaceId,
      deleteJob.expectedSha256,
      deleteJob.expectedSizeBytes,
      deleteJob.objectKey,
    ]);
    expect(markAbsent.sql).toContain("availability = excluded.availability");
    expect(markAbsent.sql).toContain("preservation.direction = 'download'");
    expect(markAbsent.expectedRowsAffected).toBe(1);
    expect(complete.sql).toContain("local.availability = 'absent'");
    expect(complete.sql).toContain("preservation.phase <> 'completed'");
    expect(complete.expectedRowsAffected).toBe(1);
  });

  it("does not retire a delete without an exact live preservation download", async () => {
    mocks.executeTransaction.mockRejectedValueOnce(
      new Error("Unexpected rows affected for statement 1"),
    );

    await expect(
      deferAttachmentTransferDeleteForPreservation({
        ...job,
        direction: "delete",
        objectKey: "owner/object.anb1",
      }),
    ).rejects.toThrow("Unexpected rows affected");
  });

  it("rejects a stale phase mutation instead of touching a newer attempt", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([0]);

    await expect(markPhase(job, "finalizing")).rejects.toThrow(
      "Attachment transfer is no longer active",
    );

    const [statement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(statement.sql).toContain("attempt_count = ?");
    expect(statement.params.slice(-2)).toEqual([job.id, job.attemptCount]);
  });

  it("fences both attachment and job completion from a stale attempt", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([0, 0, 0]);

    await expect(completeUpload(job, "owner/object.anb1")).rejects.toThrow(
      "Attachment transfer is no longer active",
    );

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(3);
    expect(statements[0].sql).toContain("job.attempt_count = ?");
    expect(statements[0].sql).toContain("job.direction = 'upload'");
    expect(statements[0].params.slice(-2)).toEqual([job.id, job.attemptCount]);
    expect(statements[2].sql).toContain("attempt_count = ?");
    expect(statements[2].params.slice(-2)).toEqual([job.id, job.attemptCount]);
  });

  it("records a promoted object so changed intent can reconcile its cleanup", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1, 0, 1]);

    await expect(completeUpload(job, "owner/object.anb1")).resolves.toBe(true);

    const [adopt] = mocks.executeTransaction.mock.calls[0]![0];
    expect(adopt.sql).not.toContain("cloud_sync_enabled = 1");
    expect(adopt.sql).not.toContain("deleted_at IS NULL");
  });

  it("does not attach a promoted object to a different attachment version", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([0, 1, 1]);

    await expect(completeUpload(job, "owner/object.anb1")).resolves.toBe(false);

    const [, cleanup] = mocks.executeTransaction.mock.calls[0]![0];
    expect(cleanup.sql).toContain("'delete'");
    expect(cleanup.sql).toContain("attachment.sha256 = job.expected_sha256");
    expect(cleanup.params).toEqual([
      "new-job-id",
      "owner/object.anb1",
      job.id,
      job.attemptCount,
      "owner/object.anb1",
    ]);
  });

  it("fences retry and terminal failure writes from a stale attempt", async () => {
    mocks.executeTransaction.mockResolvedValue([0]);

    await retryAttachmentTransferJob(job, "retry", new Date(0));
    await failAttachmentTransferJob(job, "failed");

    for (const [statements] of mocks.executeTransaction.mock.calls) {
      const [statement] = statements;
      expect(statement.sql).toContain("attempt_count = ?");
      expect(statement.params.slice(-2)).toEqual([job.id, job.attemptCount]);
    }
  });

  it("does not hand an interleaved newer claim to the stale claimant", async () => {
    mocks.execute
      .mockResolvedValueOnce([{ id: job.id, attempt_count: 3 }])
      .mockResolvedValueOnce([]);
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await expect(claimNextAttachmentTransferJob()).resolves.toBeUndefined();

    const [claim] = mocks.executeTransaction.mock.calls[0]![0];
    expect(claim.sql).toContain("attempt_count = ?");
    expect(claim.params[3]).toBe(3);
    expect(mocks.execute.mock.calls[1]![1]).toEqual([job.id, 4]);
  });
});
