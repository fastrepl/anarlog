import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn().mockResolvedValue([0, 1]),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<number[]>) => write(),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

import { catalogLocalNoteAttachment, sha256Hex } from "./attachments";

describe("attachment catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([0, 1]);
  });

  it("inherits workspace ownership and stores only a relative local path", async () => {
    await catalogLocalNoteAttachment({
      sessionId: "session-1",
      attachmentId: "diagram 1.png",
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    });

    expect(mocks.enqueueDatabaseWrite).toHaveBeenCalledWith(
      "session:session-1",
      expect.any(Function),
    );
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(2);
    expect(statements[1].sql).toContain("session.workspace_id");
    expect(statements[1].sql).toContain("session.deleted_at IS NULL");
    expect(statements[1].sql).not.toContain("/vault/");
    expect(statements[1].params).toEqual([
      expect.stringMatching(
        /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
      ),
      "diagram.png",
      "attachments/diagram 1.png",
      "image/png",
      42,
      "a".repeat(64),
      "diagram 1.png",
      "session-1",
      "attachments/diagram 1.png",
    ]);
  });

  it("updates an existing physical attachment without creating a duplicate", async () => {
    mocks.executeTransaction.mockResolvedValue([1, 0]);

    await expect(
      catalogLocalNoteAttachment({
        sessionId: "session-1",
        attachmentId: "diagram.png",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "b".repeat(64),
      }),
    ).resolves.toBeUndefined();
  });

  it("rejects missing or deleted sessions and unsafe attachment IDs", async () => {
    mocks.executeTransaction.mockResolvedValue([0, 0]);
    await expect(
      catalogLocalNoteAttachment({
        sessionId: "missing-session",
        attachmentId: "diagram.png",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "c".repeat(64),
      }),
    ).rejects.toThrow("session is unavailable");

    await expect(
      catalogLocalNoteAttachment({
        sessionId: "session-1",
        attachmentId: "../diagram.png",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "c".repeat(64),
      }),
    ).rejects.toThrow("attachment ID");

    await expect(
      catalogLocalNoteAttachment({
        sessionId: "session-1",
        attachmentId: "diagram.png",
        filename: "/vault/private/diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "c".repeat(64),
      }),
    ).rejects.toThrow("attachment filename");
  });

  it("computes a stable lowercase SHA-256 checksum", async () => {
    const bytes = new TextEncoder().encode("hello").buffer;

    await expect(sha256Hex(bytes)).resolves.toBe(
      "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    );
  });
});
