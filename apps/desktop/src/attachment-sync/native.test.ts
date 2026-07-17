import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  beginAttachmentDownload: vi.fn(),
  cancelAttachmentDownload: vi.fn(),
  verifyDeleteSource: vi.fn(),
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

describe("native attachment download cancellation", () => {
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
    mocks.verifyDeleteSource.mockResolvedValue({
      status: "ok",
      data: true,
    });
  });

  it("forwards an exact delete attempt to native source verification", async () => {
    await expect(
      attachmentTransferNative.verifyDeleteSource("job-1", 7),
    ).resolves.toBe(true);

    expect(mocks.verifyDeleteSource).toHaveBeenCalledWith("job-1", 7);
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
