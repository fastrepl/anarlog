import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  beginAttachmentDownload: vi.fn(),
  cancelAttachmentDownload: vi.fn(),
  beginSharedUploadOperation: vi.fn(),
  cancelSharedUploadOperation: vi.fn(),
  prepareSharedUpload: vi.fn(),
  validateSharedUpload: vi.fn(),
  prepareDeleteGuard: vi.fn(),
  commitDeleteGuard: vi.fn(),
  reconcileDeleteGuards: vi.fn(),
  downloadAndRestore: vi.fn(),
  downloadSharedAttachment: vi.fn(),
}));

vi.mock("@hypr/plugin-attachment-sync", () => ({
  commands: mocks,
}));

import { attachmentTransferNative } from "./native";

const privateInput = {
  jobId: "job-1",
  attemptCount: 1,
  objectId: "11111111-1111-4111-8111-111111111111",
  signedUrl: "https://project.supabase.co/private?token=one",
  ciphertextSha256: "a".repeat(64),
  ciphertextSizeBytes: 42,
  formatVersion: 1,
};

describe("native attachment operation cancellation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.beginAttachmentDownload.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.cancelAttachmentDownload.mockResolvedValue({
      status: "ok",
      data: true,
    });
    mocks.beginSharedUploadOperation.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.cancelSharedUploadOperation.mockResolvedValue({
      status: "ok",
      data: true,
    });
    mocks.prepareDeleteGuard.mockResolvedValue({
      status: "ok",
      data: {
        shouldDelete: true,
        guardId: "guard-1",
        attachmentRef: "attachment-ref",
        versionRef: "version-ref",
      },
    });
    mocks.commitDeleteGuard.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.reconcileDeleteGuards.mockResolvedValue({
      status: "ok",
      data: 0,
    });
  });

  it("registers the exact delete attempt while preparing its guard", async () => {
    await expect(
      attachmentTransferNative.prepareDeleteGuard("job-1", 7, true),
    ).resolves.toEqual({
      shouldDelete: true,
      guardId: "guard-1",
      attachmentRef: "attachment-ref",
      versionRef: "version-ref",
    });

    const operationId = mocks.beginSharedUploadOperation.mock.calls[0]?.[0];
    expect(mocks.prepareDeleteGuard).toHaveBeenCalledWith(
      operationId,
      "job-1",
      7,
      true,
    );
  });

  it("commits the exact guarded delete through the cancellable native path", async () => {
    await expect(
      attachmentTransferNative.commitDeleteGuard("job-1", 7, "guard-1"),
    ).resolves.toBeNull();

    const operationId = mocks.beginSharedUploadOperation.mock.calls[0]?.[0];
    expect(mocks.commitDeleteGuard).toHaveBeenCalledWith(
      operationId,
      "job-1",
      7,
      "guard-1",
    );
  });

  it("reconciles durable delete guards outside the cache purge path", async () => {
    await expect(
      attachmentTransferNative.reconcileDeleteGuards(),
    ).resolves.toBe(0);

    expect(mocks.reconcileDeleteGuards).toHaveBeenCalledOnce();
  });

  it("registers before starting a private download and cancels the same operation", async () => {
    const controller = new AbortController();
    let finish:
      | ((value: { status: "error"; error: string }) => void)
      | undefined;
    mocks.downloadAndRestore.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );

    const download = attachmentTransferNative.downloadAndRestore(
      privateInput,
      controller.signal,
    );
    await vi.waitFor(() => expect(mocks.downloadAndRestore).toHaveBeenCalled());
    const operationId = mocks.beginAttachmentDownload.mock.calls[0]?.[0];
    expect(mocks.beginAttachmentDownload).toHaveBeenCalledWith(
      operationId,
      null,
    );
    expect(mocks.downloadAndRestore.mock.calls[0]?.[0]).toBe(operationId);

    controller.abort();
    await vi.waitFor(() =>
      expect(mocks.cancelAttachmentDownload).toHaveBeenCalledWith(operationId),
    );
    finish?.({ status: "error", error: "attachment transfer was cancelled" });
    await expect(download).rejects.toThrow("cancelled");
  });

  it("cancels after registration when abort wins the begin race", async () => {
    const controller = new AbortController();
    const reason = new Error("stop before native start");
    let finishBegin:
      | ((value: { status: "ok"; data: null }) => void)
      | undefined;
    mocks.beginAttachmentDownload.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishBegin = resolve;
        }),
    );

    const download = attachmentTransferNative.downloadAndRestore(
      privateInput,
      controller.signal,
    );
    controller.abort(reason);
    expect(mocks.cancelAttachmentDownload).not.toHaveBeenCalled();

    finishBegin?.({ status: "ok", data: null });
    await expect(download).rejects.toBe(reason);
    const operationId = mocks.beginAttachmentDownload.mock.calls[0]?.[0];
    expect(mocks.cancelAttachmentDownload).toHaveBeenCalledWith(operationId);
    expect(mocks.downloadAndRestore).not.toHaveBeenCalled();
  });

  it("cancels a shared upload snapshot and drains its native copy", async () => {
    const controller = new AbortController();
    let finish:
      | ((value: { status: "error"; error: string }) => void)
      | undefined;
    mocks.prepareSharedUpload.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );

    const snapshot = attachmentTransferNative.prepareSharedUpload(
      "attachment-1",
      "a".repeat(64),
      42,
      "diagram.png",
      "image/png",
      "private/object.anb1",
      controller.signal,
    );
    await vi.waitFor(() =>
      expect(mocks.prepareSharedUpload).toHaveBeenCalled(),
    );
    const operationId = mocks.beginSharedUploadOperation.mock.calls[0]?.[0];
    expect(mocks.prepareSharedUpload).toHaveBeenCalledWith(
      operationId,
      "attachment-1",
      {
        sha256: "a".repeat(64),
        sizeBytes: 42,
        filename: "diagram.png",
        contentType: "image/png",
        cloudObjectKey: "private/object.anb1",
      },
    );

    controller.abort();
    await vi.waitFor(() =>
      expect(mocks.cancelSharedUploadOperation).toHaveBeenCalledWith(
        operationId,
      ),
    );
    finish?.({ status: "error", error: "attachment transfer was cancelled" });
    await expect(snapshot).rejects.toThrow("cancelled");
  });

  it("cancels and drains shared upload validation", async () => {
    const controller = new AbortController();
    let finish:
      | ((value: { status: "error"; error: string }) => void)
      | undefined;
    mocks.validateSharedUpload.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );

    const validation = attachmentTransferNative.validateSharedUpload(
      "attachment-1",
      "44444444-4444-4444-8444-444444444444",
      "a".repeat(64),
      42,
      "diagram.png",
      "image/png",
      "private/object.anb1",
      controller.signal,
    );
    await vi.waitFor(() =>
      expect(mocks.validateSharedUpload).toHaveBeenCalled(),
    );
    const operationId =
      mocks.beginSharedUploadOperation.mock.calls[
        mocks.beginSharedUploadOperation.mock.calls.length - 1
      ]?.[0];
    expect(mocks.validateSharedUpload).toHaveBeenCalledWith(
      operationId,
      "attachment-1",
      "44444444-4444-4444-8444-444444444444",
      {
        sha256: "a".repeat(64),
        sizeBytes: 42,
        filename: "diagram.png",
        contentType: "image/png",
        cloudObjectKey: "private/object.anb1",
      },
    );

    controller.abort();
    await vi.waitFor(() =>
      expect(mocks.cancelSharedUploadOperation).toHaveBeenCalledWith(
        operationId,
      ),
    );
    finish?.({ status: "error", error: "attachment transfer was cancelled" });
    await expect(validation).rejects.toThrow("cancelled");
  });

  it("registers shared downloads under their purge scope", async () => {
    mocks.downloadSharedAttachment.mockResolvedValueOnce({
      status: "ok",
      data: {
        cacheId: "cache-1",
        localPath: "/cache/file.bin",
        sizeBytes: 42,
        sha256: "b".repeat(64),
      },
    });

    await attachmentTransferNative.downloadSharedAttachment({
      scopeId: "viewer-1",
      attachmentId: "22222222-2222-4222-8222-222222222222",
      signedUrl: "https://project.supabase.co/shared?token=one",
      expectedSha256: "b".repeat(64),
      expectedSizeBytes: 42,
    });

    const operationId = mocks.beginAttachmentDownload.mock.calls[0]?.[0];
    expect(mocks.beginAttachmentDownload).toHaveBeenCalledWith(
      operationId,
      "viewer-1",
    );
    expect(mocks.downloadSharedAttachment.mock.calls[0]?.[0]).toBe(operationId);
    expect(mocks.cancelAttachmentDownload).toHaveBeenCalledWith(operationId);
  });
});
