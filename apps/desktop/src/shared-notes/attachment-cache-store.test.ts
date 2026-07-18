import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<unknown>) => write(),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: vi.fn() },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
  flushDatabaseWrites: vi.fn(),
}));

import {
  purgeViewerSharedNoteCache,
  type SharedAttachmentCacheJob,
  sharedAttachmentCacheStore,
} from "./attachment-cache-store";

const job: SharedAttachmentCacheJob = {
  viewerUserId: "viewer-1",
  shareId: "11111111-1111-4111-8111-111111111111",
  attachmentId: "22222222-2222-4222-8222-222222222222",
  filename: "diagram.png",
  contentType: "image/png",
  sizeBytes: 42,
  sha256: "a".repeat(64),
  cacheId: "cache-1",
  claimToken: "claim-1",
  availability: "deleting",
  accessVersion: 3,
  attemptCount: 1,
};

describe("shared attachment cache store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("invalidates a same-generation row resurrected while its bytes were deleted", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([0, 1]);

    await expect(sharedAttachmentCacheStore.completeDelete(job)).resolves.toBe(
      true,
    );

    const [, invalidate] = mocks.executeTransaction.mock.calls[0]![0];
    expect(invalidate.sql).toContain("availability = 'pending'");
    expect(invalidate.sql).toContain("cache_generation = cache_generation + 1");
    expect(invalidate.sql).toContain("availability = 'present'");
    expect(invalidate.sql).toContain("cache_id = ?");
    expect(invalidate.params).toEqual([
      job.viewerUserId,
      job.shareId,
      job.attachmentId,
      job.cacheId,
    ]);
  });

  it("serializes viewer purges with full shared-note cache replacements", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([]);
    const removeScope = vi.fn(async () => {});
    const signal = new AbortController().signal;

    await purgeViewerSharedNoteCache(job.viewerUserId, removeScope, signal);

    expect(removeScope).toHaveBeenCalledWith(job.viewerUserId);
    expect(mocks.enqueueDatabaseWrite.mock.calls.map(([key]) => key)).toEqual([
      `shared-note-cache:${job.viewerUserId}`,
      "shared-attachment-cache-runner",
    ]);
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(2);
    expect(statements[1].sql).toContain("DELETE FROM shared_session_cache");
  });

  it("does not let a stopped account purge clear a viewer that became active while queued", async () => {
    let runQueuedPurge: (() => void) | undefined;
    mocks.enqueueDatabaseWrite.mockImplementationOnce(
      (_key, write) =>
        new Promise((resolve, reject) => {
          runQueuedPurge = () => {
            void write().then(resolve, reject);
          };
        }),
    );
    const controller = new AbortController();
    const removeScope = vi.fn(async () => {});

    const purge = purgeViewerSharedNoteCache(
      "viewer-b",
      removeScope,
      controller.signal,
    );
    await vi.waitFor(() => expect(runQueuedPurge).toBeTypeOf("function"));
    controller.abort();
    runQueuedPurge?.();

    await expect(purge).rejects.toMatchObject({ name: "AbortError" });
    expect(removeScope).not.toHaveBeenCalled();
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("preserves durable rows when the account switches during native clearing", async () => {
    const controller = new AbortController();
    const removeScope = vi.fn(async () => controller.abort());

    await expect(
      purgeViewerSharedNoteCache("viewer-b", removeScope, controller.signal),
    ).rejects.toMatchObject({ name: "AbortError" });

    expect(removeScope).toHaveBeenCalledWith("viewer-b");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });
});
