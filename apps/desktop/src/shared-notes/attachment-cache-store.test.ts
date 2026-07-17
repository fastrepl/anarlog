import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<number[]>) => write(),
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
});
