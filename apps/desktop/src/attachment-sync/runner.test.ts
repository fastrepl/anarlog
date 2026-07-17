import { describe, expect, it, vi } from "vitest";

import { runAttachmentTransferJob, runAttachmentTransferPass } from "./runner";
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
    recoverInterrupted: vi.fn(),
    reconcile: vi.fn(),
    claimNext: vi.fn(),
    setUploadReservation: vi.fn(),
    setDownloadGrant: vi.fn(),
    markPhase: vi.fn(),
    completeUpload: vi.fn().mockResolvedValue(true),
    completeDelete: vi.fn(),
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
    delete: vi.fn(),
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
    downloadAndRestore: vi.fn(),
    cleanupTransferCache: vi.fn(),
  };
  const uploader = vi.fn(() => ({
    promise: Promise.resolve("owner/object.anb1"),
    abort: vi.fn(),
  }));
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
      job.id,
      "transferring",
    );
    expect(deps.store.markPhase).toHaveBeenNthCalledWith(
      2,
      job.id,
      "finalizing",
    );
    expect(deps.client.finalize).toHaveBeenCalledWith(
      "owner/object.anb1",
      undefined,
    );
    expect(deps.store.completeUpload).toHaveBeenCalledWith(
      job,
      "owner/object.anb1",
    );
    expect(deps.native.cleanupTransferCache).toHaveBeenCalledWith(job.id);
  });

  it("skips an upload whose attachment intent changed before execution", async () => {
    const deps = dependencies();

    await runAttachmentTransferJob(
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      { ...job, cloudSyncEnabled: false },
    );

    expect(deps.store.completeWithoutTransfer).toHaveBeenCalledWith(job.id);
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
    expect(deps.native.cleanupTransferCache).toHaveBeenCalledWith(job.id);
  });

  it("cleans an interrupted upload cache when the reservation is already current", async () => {
    const deps = dependencies();
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
      job,
    );

    expect(deps.client.head).not.toHaveBeenCalled();
    expect(deps.client.promote).not.toHaveBeenCalled();
    expect(deps.native.prepareUpload).not.toHaveBeenCalled();
    expect(deps.store.completeUpload).toHaveBeenCalledWith(
      job,
      "owner/object.anb1",
    );
    expect(deps.native.cleanupTransferCache).toHaveBeenCalledWith(job.id);
  });

  it("uses the native atomic restore as the download completion boundary", async () => {
    const deps = dependencies();
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
      { ...deps, supabaseUrl: "https://project.supabase.co" } as any,
      downloadJob,
    );

    expect(deps.store.setDownloadGrant).toHaveBeenCalledOnce();
    expect(deps.native.downloadAndRestore).toHaveBeenCalledOnce();
    expect(deps.store.completeWithoutTransfer).not.toHaveBeenCalled();
    expect(deps.native.cleanupTransferCache).toHaveBeenCalledWith(job.id);
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
});
