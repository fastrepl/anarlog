import { describe, expect, it, vi } from "vitest";

import {
  AttachmentBackupGatewayError,
  type AttachmentBackupDeleteRequest,
} from "./client";
import { NativeAttachmentTransferError } from "./native";
import {
  runAttachmentTransferJob,
  runAttachmentTransferPass,
  startAttachmentTransferRunner,
} from "./runner";
import type { AttachmentTransferJob } from "./store";

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
  cacheId: "",
  phase: "preparing",
  attemptCount: 1,
  cloudSyncEnabled: true,
  currentObjectKey: "",
  attachmentDeleted: false,
  localAvailability: "present",
  attachmentVersionMatches: true,
};

function dependencies() {
  const store = {
    resetProcessLocalAttempts: vi.fn(),
    recoverInterrupted: vi.fn(),
    reconcile: vi.fn(),
    claimNext: vi.fn(),
    setUploadReservation: vi.fn(),
    setDownloadGrant: vi.fn(),
    markPhase: vi.fn(),
    prepareDelete: vi.fn().mockResolvedValue(true),
    completeCancelledDelete: vi.fn(),
    deferDeleteForPreservation: vi.fn(),
    completeUpload: vi.fn().mockResolvedValue(true),
    completeWithoutTransfer: vi.fn(),
    retry: vi.fn(),
    fail: vi.fn(),
  };
  const client = {
    reserve: vi.fn().mockResolvedValue({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      objectState: "reserved",
      ciphertextSizeBytes: 58,
      formatVersion: 1,
      ciphertextSha256: null,
    }),
    grantUpload: vi.fn().mockResolvedValue({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      objectState: "reserved",
      ciphertextSizeBytes: 58,
      ciphertextSha256: "b".repeat(64),
      formatVersion: 1,
      uploadExpiresAt: null,
      uploadToken: "signed-token",
    }),
    finalize: vi.fn(),
    head: vi.fn().mockResolvedValue(null),
    promote: vi.fn().mockResolvedValue({
      currentObjectKey: "owner/object.anb1",
      currentVersionRef: "version-ref",
      currentCiphertextSha256: "b".repeat(64),
      displacedObjectKey: null,
      wasPromoted: true,
    }),
    download: vi.fn(),
    scheduleDelete: vi.fn(async (input: AttachmentBackupDeleteRequest) => ({
      ...input,
      deleteFenceId: "fence-1",
      deleteGeneration: 7,
      deleteNotBefore: "2026-07-19T12:00:00.000Z",
    })),
    cancelDelete: vi.fn(async (input: AttachmentBackupDeleteRequest) => input),
  };
  const native = {
    describeUpload: vi.fn().mockResolvedValue({
      attachmentRef: "attachment-ref",
      versionRef: "version-ref",
      ciphertextSizeBytes: 58,
      formatVersion: 1,
    }),
    prepareUpload: vi.fn().mockResolvedValue({
      cacheId: "cache-1",
      ciphertextSha256: "b".repeat(64),
      ciphertextSizeBytes: 58,
    }),
    readUploadRange: vi.fn(),
    prepareDeleteGuard: vi.fn().mockResolvedValue({
      shouldDelete: true,
      guardId: "guard-1",
      attachmentRef: "attachment-ref",
      versionRef: "version-ref",
    }),
    commitDeleteGuard: vi.fn(),
    reconcileDeleteGuards: vi.fn().mockResolvedValue(0),
    downloadAndRestore: vi.fn(),
    cleanupTransferCache: vi.fn(),
  };
  const uploader = vi.fn(
    (_input: {
      readRange: (start: number, end: number) => Promise<unknown>;
    }) => ({
      promise: Promise.resolve("owner/object.anb1"),
      abort: vi.fn(),
    }),
  );
  return { store, client, native, uploader };
}

describe("attachment transfer runner", () => {
  it("uploads, finalizes, promotes, and commits the private backup", async () => {
    const deps = dependencies();

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      job,
    );

    expect(deps.store.markPhase).toHaveBeenNthCalledWith(
      1,
      job,
      "transferring",
    );
    expect(deps.store.markPhase).toHaveBeenNthCalledWith(2, job, "finalizing");
    expect(deps.client.finalize).toHaveBeenCalledWith(
      "owner/object.anb1",
      undefined,
    );
    expect(deps.store.completeUpload).toHaveBeenCalledWith(
      job,
      "owner/object.anb1",
    );
    expect(deps.native.describeUpload).toHaveBeenCalledWith(
      job.id,
      job.attemptCount,
    );
    expect(deps.native.prepareUpload).toHaveBeenCalledWith(
      job.id,
      job.attemptCount,
      "object-1",
      "owner/object.anb1",
    );
    const uploadInput = deps.uploader.mock.calls[0]![0];
    await uploadInput.readRange(0, 16);
    expect(deps.native.readUploadRange).toHaveBeenCalledWith(
      job.id,
      job.attemptCount,
      "cache-1",
      0,
      16,
    );
    expect(deps.native.cleanupTransferCache).toHaveBeenCalledWith(
      job.id,
      job.attemptCount,
      "cache-1",
    );
  });

  it("skips an upload whose attachment intent changed before execution", async () => {
    const deps = dependencies();

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      { ...job, cloudSyncEnabled: false },
    );

    expect(deps.store.completeWithoutTransfer).toHaveBeenCalledWith({
      ...job,
      cloudSyncEnabled: false,
    });
    expect(deps.client.reserve).not.toHaveBeenCalled();
  });

  it("promotes an already-ready reservation without resealing or uploading", async () => {
    const deps = dependencies();
    deps.client.reserve.mockResolvedValueOnce({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      objectState: "ready",
      ciphertextSizeBytes: 58,
      formatVersion: 1,
      ciphertextSha256: "b".repeat(64),
    });

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      job,
    );

    expect(deps.native.prepareUpload).not.toHaveBeenCalled();
    expect(deps.client.grantUpload).not.toHaveBeenCalled();
    expect(deps.uploader).not.toHaveBeenCalled();
    expect(deps.client.finalize).not.toHaveBeenCalled();
    expect(deps.client.promote).toHaveBeenCalledWith(
      {
        objectKey: "owner/object.anb1",
        expectedCurrentObjectKey: null,
      },
      undefined,
    );
    expect(deps.store.completeUpload).toHaveBeenCalledWith(
      job,
      "owner/object.anb1",
    );
    expect(deps.native.cleanupTransferCache).not.toHaveBeenCalled();
  });

  it("cleans an interrupted upload cache when the reservation is already current", async () => {
    const deps = dependencies();
    const interruptedJob = { ...job, cacheId: "cache-old" };
    deps.client.reserve.mockResolvedValueOnce({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      objectState: "current",
      ciphertextSizeBytes: 58,
      formatVersion: 1,
      ciphertextSha256: "b".repeat(64),
    });

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      interruptedJob,
    );

    expect(deps.client.head).not.toHaveBeenCalled();
    expect(deps.client.promote).not.toHaveBeenCalled();
    expect(deps.native.prepareUpload).not.toHaveBeenCalled();
    expect(deps.store.completeUpload).toHaveBeenCalledWith(
      interruptedJob,
      "owner/object.anb1",
    );
    expect(deps.native.cleanupTransferCache).toHaveBeenCalledWith(
      interruptedJob.id,
      interruptedJob.attemptCount,
      interruptedJob.cacheId,
    );
  });

  it("does not delete a current object when local attachment intent changes", async () => {
    const deps = dependencies();
    deps.client.reserve.mockResolvedValueOnce({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      objectState: "current",
      ciphertextSizeBytes: 58,
      formatVersion: 1,
      ciphertextSha256: "b".repeat(64),
    });
    deps.store.completeUpload.mockResolvedValueOnce(false);

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      job,
    );

    expect(deps.client.scheduleDelete).not.toHaveBeenCalled();
  });

  it("does not delete a current object when upload completion is stale", async () => {
    const deps = dependencies();
    deps.client.reserve.mockResolvedValueOnce({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      objectState: "current",
      ciphertextSizeBytes: 58,
      formatVersion: 1,
      ciphertextSha256: "b".repeat(64),
    });
    deps.store.completeUpload.mockRejectedValueOnce(
      new Error("Attachment transfer is no longer active"),
    );

    await expect(
      runAttachmentTransferJob(
        { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
        job,
      ),
    ).rejects.toThrow("Attachment transfer is no longer active");

    expect(deps.client.scheduleDelete).not.toHaveBeenCalled();
  });

  it("uses the native atomic restore as the download completion boundary", async () => {
    const deps = dependencies();
    const controller = new AbortController();
    const onAttachmentRestored = vi.fn();
    const downloadJob = {
      ...job,
      direction: "download" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      localAvailability: "absent" as const,
    };
    deps.client.download.mockResolvedValueOnce({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      ciphertextSizeBytes: 58,
      ciphertextSha256: "b".repeat(64),
      formatVersion: 1,
      signedUrl:
        "https://project.supabase.co/storage/v1/object/sign/attachment-backups/owner/object.anb1?token=secret",
      expiresAt: "2026-07-17T12:00:00.000Z",
    });
    deps.native.downloadAndRestore.mockResolvedValueOnce({
      attachmentId: job.attachmentId,
      sessionId: job.sessionId,
      relativePath: "attachments/file.bin",
      sizeBytes: job.expectedSizeBytes,
      sha256: job.expectedSha256,
    });

    await runAttachmentTransferJob(
      {
        ...deps,
        supabaseUrl: "https://project.supabase.co",
        onAttachmentRestored,
      } as any,
      downloadJob,
      controller.signal,
    );

    expect(deps.store.setDownloadGrant).toHaveBeenCalledOnce();
    expect(deps.store.setDownloadGrant).toHaveBeenCalledWith(
      downloadJob,
      expect.objectContaining({ objectId: "object-1" }),
    );
    expect(deps.native.downloadAndRestore).toHaveBeenCalledOnce();
    expect(deps.native.downloadAndRestore.mock.calls[0]?.[1]).toBe(
      controller.signal,
    );
    expect(deps.store.completeWithoutTransfer).not.toHaveBeenCalled();
    expect(onAttachmentRestored).toHaveBeenCalledWith({
      attachmentId: job.attachmentId,
      sessionId: job.sessionId,
      relativePath: "attachments/file.bin",
      sizeBytes: job.expectedSizeBytes,
      sha256: job.expectedSha256,
    });
    expect(deps.native.cleanupTransferCache).not.toHaveBeenCalled();
  });

  it("restores a cloud-only attachment before deleting its disabled backup", async () => {
    const deps = dependencies();
    const downloadJob = {
      ...job,
      direction: "download" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      localAvailability: "absent" as const,
      cloudSyncEnabled: false,
    };
    deps.client.download.mockResolvedValueOnce({
      objectId: "object-1",
      objectKey: "owner/object.anb1",
      ciphertextSizeBytes: 58,
      ciphertextSha256: "b".repeat(64),
      formatVersion: 1,
      signedUrl:
        "https://project.supabase.co/storage/v1/object/sign/attachment-backups/owner/object.anb1?token=secret",
      expiresAt: "2026-07-17T12:00:00.000Z",
    });
    deps.native.downloadAndRestore.mockResolvedValueOnce({
      attachmentId: job.attachmentId,
      sessionId: job.sessionId,
      relativePath: "attachments/file.bin",
      sizeBytes: job.expectedSizeBytes,
      sha256: job.expectedSha256,
    });

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      downloadJob,
    );

    expect(deps.client.download).toHaveBeenCalledOnce();
    expect(deps.native.downloadAndRestore).toHaveBeenCalledOnce();
    expect(deps.store.completeWithoutTransfer).not.toHaveBeenCalled();
  });

  it("cancels before completing a superseded delete", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    deps.store.prepareDelete.mockResolvedValueOnce(false);
    deps.native.prepareDeleteGuard.mockImplementationOnce(
      async (_jobId: string, _attemptCount: number, createGuard: boolean) => {
        if (createGuard) throw new Error("unexpected guard hashing");
        return {
          shouldDelete: false,
          guardId: "",
          attachmentRef: "attachment-ref",
          versionRef: "version-ref",
        };
      },
    );

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      deleteJob,
    );

    expect(deps.store.prepareDelete).toHaveBeenCalledWith(deleteJob);
    expect(deps.native.prepareDeleteGuard).toHaveBeenCalledWith(
      deleteJob.id,
      deleteJob.attemptCount,
      false,
      undefined,
    );
    expect(deps.client.cancelDelete).toHaveBeenCalledWith(
      {
        objectKey: deleteJob.objectKey,
        attachmentRef: "attachment-ref",
        versionRef: "version-ref",
        deleteRequestId: deleteJob.id,
      },
      undefined,
    );
    expect(deps.store.completeCancelledDelete).toHaveBeenCalledWith(deleteJob);
    expect(deps.client.cancelDelete.mock.invocationCallOrder[0]).toBeLessThan(
      deps.store.completeCancelledDelete.mock.invocationCallOrder[0]!,
    );
    expect(deps.client.scheduleDelete).not.toHaveBeenCalled();
    expect(deps.native.commitDeleteGuard).not.toHaveBeenCalled();
  });

  it("keeps a superseded delete retryable when cancellation fails", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    deps.store.prepareDelete.mockResolvedValueOnce(false);
    deps.client.cancelDelete.mockRejectedValueOnce(
      new Error("cancellation unavailable"),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(deleteJob)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.store.completeCancelledDelete).not.toHaveBeenCalled();
    expect(deps.store.retry).toHaveBeenCalledWith(
      deleteJob,
      "cancellation unavailable",
      expect.any(Date),
    );
  });

  it("retains a verified guard through remote deletion and native commit", async () => {
    const deps = dependencies();
    let sourceMutated = false;
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    deps.client.scheduleDelete.mockImplementationOnce(async (input) => {
      sourceMutated = true;
      return {
        ...input,
        deleteFenceId: "fence-1",
        deleteGeneration: 7,
        deleteNotBefore: "2026-07-19T12:00:00.000Z",
      };
    });
    deps.native.commitDeleteGuard.mockImplementationOnce(async () => {
      expect(sourceMutated).toBe(true);
    });

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      deleteJob,
    );

    expect(deps.native.prepareDeleteGuard).toHaveBeenCalledWith(
      deleteJob.id,
      deleteJob.attemptCount,
      true,
      undefined,
    );
    expect(deps.client.scheduleDelete).toHaveBeenCalledWith(
      {
        objectKey: deleteJob.objectKey,
        attachmentRef: "attachment-ref",
        versionRef: "version-ref",
        deleteRequestId: deleteJob.id,
      },
      undefined,
    );
    expect(deps.native.commitDeleteGuard).toHaveBeenCalledWith(
      deleteJob.id,
      deleteJob.attemptCount,
      "guard-1",
      undefined,
    );
    expect(
      deps.native.prepareDeleteGuard.mock.invocationCallOrder[0],
    ).toBeLessThan(deps.client.scheduleDelete.mock.invocationCallOrder[0]!);
    expect(deps.client.scheduleDelete.mock.invocationCallOrder[0]).toBeLessThan(
      deps.native.commitDeleteGuard.mock.invocationCallOrder[0]!,
    );
  });

  it("retries with the linked guard when native commit fails after deletion", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    deps.native.commitDeleteGuard.mockRejectedValueOnce(
      new NativeAttachmentTransferError(
        "commit attachment delete guard",
        "attachment delete guard changed during commit",
      ),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(deleteJob)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.client.scheduleDelete).toHaveBeenCalledOnce();
    expect(deps.native.commitDeleteGuard).toHaveBeenCalledOnce();
    expect(deps.store.retry).toHaveBeenCalledWith(
      deleteJob,
      "commit attachment delete guard failed: attachment delete guard changed during commit",
      expect.any(Date),
    );
  });

  it("commits locally on the typed dependency conflict without chasing head", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-object.anb1",
      currentObjectKey: "owner/old-object.anb1",
      cloudSyncEnabled: false,
    };
    deps.client.scheduleDelete.mockRejectedValueOnce(
      new AttachmentBackupGatewayError(
        409,
        "attachment_backup_dependency_appeared",
      ),
    );

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      deleteJob,
    );

    expect(deps.native.commitDeleteGuard).toHaveBeenCalledOnce();
    expect(deps.client.head).not.toHaveBeenCalled();
    expect(deps.client.scheduleDelete).toHaveBeenCalledOnce();
  });

  it("retires a cancelled replay without committing its local guard", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-object.anb1",
      currentObjectKey: "owner/old-object.anb1",
      cloudSyncEnabled: false,
    };
    deps.client.scheduleDelete.mockRejectedValueOnce(
      new AttachmentBackupGatewayError(
        409,
        "attachment_backup_delete_cancelled",
      ),
    );

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      deleteJob,
    );

    expect(deps.store.completeCancelledDelete).toHaveBeenCalledWith(deleteJob);
    expect(deps.native.commitDeleteGuard).not.toHaveBeenCalled();
  });

  it("retries a cancelled replay when local completion fails", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-object.anb1",
      currentObjectKey: "owner/old-object.anb1",
      cloudSyncEnabled: false,
    };
    deps.client.scheduleDelete.mockRejectedValueOnce(
      new AttachmentBackupGatewayError(
        409,
        "attachment_backup_delete_cancelled",
      ),
    );
    deps.store.completeCancelledDelete.mockRejectedValueOnce(
      new Error("cancelled delete changed locally"),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(deleteJob)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.native.commitDeleteGuard).not.toHaveBeenCalled();
    expect(deps.store.retry).toHaveBeenCalledWith(
      deleteJob,
      "cancelled delete changed locally",
      expect.any(Date),
    );
  });

  it("fails a too-late cancellation without completing locally", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-object.anb1",
      currentObjectKey: "owner/old-object.anb1",
      cloudSyncEnabled: false,
    };
    deps.store.prepareDelete.mockResolvedValueOnce(false);
    deps.client.cancelDelete.mockRejectedValueOnce(
      new AttachmentBackupGatewayError(
        409,
        "attachment_backup_delete_too_late",
      ),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(deleteJob)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.store.completeCancelledDelete).not.toHaveBeenCalled();
    expect(deps.store.fail).toHaveBeenCalledWith(
      deleteJob,
      "Attachment backup request failed (409: attachment_backup_delete_too_late)",
    );
    expect(deps.store.retry).not.toHaveBeenCalled();
  });

  it("retries a generic delete conflict without committing locally", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-object.anb1",
      currentObjectKey: "owner/old-object.anb1",
      cloudSyncEnabled: false,
    };
    deps.client.scheduleDelete.mockRejectedValueOnce(
      new AttachmentBackupGatewayError(409, "attachment_backup_conflict"),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(deleteJob)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.native.commitDeleteGuard).not.toHaveBeenCalled();
    expect(deps.store.retry).toHaveBeenCalledWith(
      deleteJob,
      "Attachment backup request failed (409: attachment_backup_conflict)",
      expect.any(Date),
    );
  });

  it("keeps the delete request identity stable across attempts", async () => {
    const deps = dependencies();
    const first = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/old-object.anb1",
      currentObjectKey: "owner/old-object.anb1",
      cloudSyncEnabled: false,
    };
    const second = { ...first, attemptCount: first.attemptCount + 1 };

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      first,
    );
    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      second,
    );

    expect(deps.client.scheduleDelete).toHaveBeenCalledTimes(2);
    for (const [request] of deps.client.scheduleDelete.mock.calls) {
      expect(request).toMatchObject({
        objectKey: first.objectKey,
        attachmentRef: "attachment-ref",
        versionRef: "version-ref",
        deleteRequestId: first.id,
      });
    }
    expect(deps.client.head).not.toHaveBeenCalled();
  });

  it("retries when the delete source changes while preparing its guard", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    deps.native.prepareDeleteGuard.mockRejectedValueOnce(
      new NativeAttachmentTransferError(
        "prepare attachment delete guard",
        "attachment delete guard changed during commit",
      ),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(deleteJob)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.client.scheduleDelete).not.toHaveBeenCalled();
    expect(deps.store.retry).toHaveBeenCalledWith(
      deleteJob,
      "prepare attachment delete guard failed: attachment delete guard changed during commit",
      expect.any(Date),
    );
    expect(deps.store.fail).not.toHaveBeenCalled();
  });

  it("preserves the remote object when the exact local source does not match", async () => {
    const deps = dependencies();
    const deleteJob = {
      ...job,
      direction: "delete" as const,
      objectKey: "owner/object.anb1",
      currentObjectKey: "owner/object.anb1",
      cloudSyncEnabled: false,
    };
    deps.native.prepareDeleteGuard.mockResolvedValueOnce({
      shouldDelete: false,
      guardId: "",
      attachmentRef: "attachment-ref",
      versionRef: "version-ref",
    });

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      deleteJob,
    );

    expect(deps.store.deferDeleteForPreservation).toHaveBeenCalledWith(
      deleteJob,
    );
    expect(deps.client.cancelDelete).toHaveBeenCalledWith(
      {
        objectKey: deleteJob.objectKey,
        attachmentRef: "attachment-ref",
        versionRef: "version-ref",
        deleteRequestId: deleteJob.id,
      },
      undefined,
    );
    expect(deps.client.scheduleDelete).not.toHaveBeenCalled();
    expect(deps.native.commitDeleteGuard).not.toHaveBeenCalled();
  });

  it("moves transient failures to durable retry wait", async () => {
    const deps = dependencies();
    deps.client.reserve.mockRejectedValueOnce(new Error("network unavailable"));
    deps.store.claimNext
      .mockResolvedValueOnce(job)
      .mockResolvedValueOnce(undefined);

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.store.retry).toHaveBeenCalledWith(
      job,
      "network unavailable",
      expect.any(Date),
    );
    expect(deps.store.fail).not.toHaveBeenCalled();
  });

  it("invalidates process-local attempts before the first transfer pass", async () => {
    const deps = dependencies();
    deps.store.claimNext.mockResolvedValue(undefined);

    const stop = startAttachmentTransferRunner({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    await vi.waitFor(() =>
      expect(deps.store.resetProcessLocalAttempts).toHaveBeenCalledOnce(),
    );
    await vi.waitFor(() =>
      expect(deps.store.recoverInterrupted).toHaveBeenCalled(),
    );
    expect(deps.native.reconcileDeleteGuards).toHaveBeenCalledOnce();
    expect(
      deps.store.resetProcessLocalAttempts.mock.invocationCallOrder[0],
    ).toBeLessThan(deps.store.recoverInterrupted.mock.invocationCallOrder[0]!);
    expect(
      deps.store.resetProcessLocalAttempts.mock.invocationCallOrder[0],
    ).toBeLessThan(
      deps.native.reconcileDeleteGuards.mock.invocationCallOrder[0]!,
    );
    stop();
  });

  it("does not repeat the startup reset when the runner remounts", async () => {
    const deps = dependencies();
    deps.store.claimNext.mockResolvedValue(undefined);

    const firstStop = startAttachmentTransferRunner({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);
    await vi.waitFor(() =>
      expect(deps.store.resetProcessLocalAttempts).toHaveBeenCalledOnce(),
    );
    firstStop();

    const secondStop = startAttachmentTransferRunner({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);
    await vi.waitFor(() =>
      expect(deps.store.recoverInterrupted).toHaveBeenCalledTimes(2),
    );

    expect(deps.store.resetProcessLocalAttempts).toHaveBeenCalledOnce();
    expect(deps.native.reconcileDeleteGuards).toHaveBeenCalledOnce();
    secondStop();
  });

  it("reconciles delete guards again after the orphan grace interval", async () => {
    vi.useFakeTimers();
    const deps = dependencies();
    deps.store.claimNext.mockResolvedValue(undefined);

    try {
      const stop = startAttachmentTransferRunner({
        ...deps,
        supabaseUrl: "https://project.supabase.co",
      } as any);
      await vi.advanceTimersByTimeAsync(0);
      expect(deps.native.reconcileDeleteGuards).toHaveBeenCalledOnce();

      await vi.advanceTimersByTimeAsync(15 * 60 * 1000);
      expect(deps.native.reconcileDeleteGuards).toHaveBeenCalledTimes(2);
      stop();
    } finally {
      vi.useRealTimers();
    }
  });

  it("excludes a live process-local attempt from recurring recovery", async () => {
    const deps = dependencies();
    let rejectDescriptor: ((error: Error) => void) | undefined;
    deps.native.describeUpload.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          rejectDescriptor = reject;
        }),
    );
    deps.store.claimNext
      .mockResolvedValueOnce(job)
      .mockResolvedValue(undefined);

    const firstPass = runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);
    await vi.waitFor(() =>
      expect(deps.native.describeUpload).toHaveBeenCalledOnce(),
    );

    await runAttachmentTransferPass({
      ...deps,
      supabaseUrl: "https://project.supabase.co",
    } as any);

    expect(deps.store.recoverInterrupted).toHaveBeenNthCalledWith(2, [
      { id: job.id, attemptCount: job.attemptCount },
    ]);

    rejectDescriptor?.(new Error("stop test transfer"));
    await firstPass;
  });
});
