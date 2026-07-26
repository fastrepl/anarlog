import { describe, expect, it, vi } from "vitest";

import {
  runSharedAttachmentCacheJob,
  runSharedAttachmentCachePass,
} from "./attachment-cache-runner";
import { SharedAttachmentGatewayError } from "./attachment-client";

const job = {
  viewerUserId: "viewer-1",
  shareId: "11111111-1111-4111-8111-111111111111",
  attachmentId: "22222222-2222-4222-8222-222222222222",
  filename: "diagram.png",
  contentType: "image/png",
  sizeBytes: 42,
  sha256: "a".repeat(64),
  cacheId: "",
  claimToken: "claim-1",
  availability: "downloading" as const,
  accessVersion: 3,
  attemptCount: 1,
};

function dependencies() {
  return {
    viewerUserId: job.viewerUserId,
    client: {
      download: vi.fn().mockResolvedValue({
        id: job.attachmentId,
        filename: job.filename,
        contentType: job.contentType,
        sizeBytes: job.sizeBytes,
        sha256: job.sha256,
        signedUrl:
          "https://project.supabase.co/storage/v1/object/sign/shared/file?token=one",
        expiresAt: "2026-07-17T12:01:00.000Z",
      }),
    },
    native: {
      downloadSharedAttachment: vi.fn().mockResolvedValue({
        cacheId: "cache-1",
        localPath: "/cache/file.bin",
        sizeBytes: job.sizeBytes,
        sha256: job.sha256,
      }),
      removeSharedAttachment: vi.fn().mockResolvedValue(true),
      clearSharedAttachmentScope: vi.fn().mockResolvedValue(0),
    },
    store: {
      recoverInterrupted: vi.fn().mockResolvedValue(undefined),
      claimNext: vi.fn().mockResolvedValue(null),
      completeDownload: vi.fn().mockResolvedValue(true),
      completeDelete: vi.fn().mockResolvedValue(true),
      retry: vi.fn().mockResolvedValue(undefined),
      markDeletePending: vi.fn().mockResolvedValue(undefined),
    },
  };
}

describe("shared attachment cache runner", () => {
  it("downloads only a grant that exactly matches the cached manifest", async () => {
    const deps = dependencies();

    await runSharedAttachmentCacheJob(deps as never, job);

    expect(deps.native.downloadSharedAttachment).toHaveBeenCalledWith(
      {
        scopeId: job.viewerUserId,
        attachmentId: job.attachmentId,
        signedUrl: expect.stringContaining("project.supabase.co"),
        expectedSha256: job.sha256,
        expectedSizeBytes: job.sizeBytes,
      },
      undefined,
    );
    expect(deps.store.completeDownload).toHaveBeenCalledWith(job, "cache-1");
  });

  it("removes bytes when the cache row changed during a download", async () => {
    const deps = dependencies();
    deps.store.completeDownload.mockResolvedValueOnce(false);

    await runSharedAttachmentCacheJob(deps as never, job);

    expect(deps.native.removeSharedAttachment).toHaveBeenCalledWith(
      job.viewerUserId,
      job.attachmentId,
    );
  });

  it("removes bytes completed after the cache runner stops", async () => {
    const deps = dependencies();
    const controller = new AbortController();
    let finishDownload:
      | ((value: {
          cacheId: string;
          localPath: string;
          sizeBytes: number;
          sha256: string;
        }) => void)
      | undefined;
    deps.native.downloadSharedAttachment.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishDownload = resolve;
        }),
    );

    const transfer = runSharedAttachmentCacheJob(
      deps as never,
      job,
      controller.signal,
    );
    await vi.waitFor(() =>
      expect(deps.native.downloadSharedAttachment).toHaveBeenCalledOnce(),
    );
    expect(deps.native.downloadSharedAttachment.mock.calls[0]?.[1]).toBe(
      controller.signal,
    );
    controller.abort();
    finishDownload?.({
      cacheId: "cache-1",
      localPath: "/cache/file.bin",
      sizeBytes: job.sizeBytes,
      sha256: job.sha256,
    });

    await expect(transfer).rejects.toThrow();
    expect(deps.native.removeSharedAttachment).toHaveBeenCalledWith(
      job.viewerUserId,
      job.attachmentId,
    );
    expect(deps.store.completeDownload).not.toHaveBeenCalled();
  });

  it("rejects a mismatched grant before downloading bytes", async () => {
    const deps = dependencies();
    deps.client.download.mockResolvedValueOnce({
      ...(await deps.client.download()),
      sha256: "b".repeat(64),
    });

    await expect(
      runSharedAttachmentCacheJob(deps as never, job),
    ).rejects.toThrow("manifest");
    expect(deps.native.downloadSharedAttachment).not.toHaveBeenCalled();
  });

  it("turns a revoked download grant into durable cache deletion", async () => {
    const deps = dependencies();
    deps.store.claimNext.mockResolvedValueOnce(job).mockResolvedValueOnce(null);
    deps.client.download.mockRejectedValueOnce(
      new SharedAttachmentGatewayError(404),
    );

    await expect(runSharedAttachmentCachePass(deps as never)).resolves.toBe(1);
    expect(deps.store.markDeletePending).toHaveBeenCalledWith(job);
    expect(deps.store.retry).not.toHaveBeenCalled();
  });

  it("deletes revoked cache bytes before removing their durable row", async () => {
    const deps = dependencies();
    const deleteJob = { ...job, availability: "deleting" as const };

    await runSharedAttachmentCacheJob(deps as never, deleteJob);

    expect(deps.native.removeSharedAttachment).toHaveBeenCalledWith(
      job.viewerUserId,
      job.attachmentId,
    );
    expect(deps.store.completeDelete).toHaveBeenCalledWith(deleteJob);
  });
});
