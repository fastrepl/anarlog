import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  audioDelete: vi.fn(),
  audioMetadata: vi.fn(),
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([0, 1, 1]),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<number[]>) => write(),
  ),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioDelete: mocks.audioDelete,
    audioMetadata: mocks.audioMetadata,
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

import {
  catalogLocalNoteAttachment,
  catalogLocalSessionAudio,
  cleanupDeletedSessionAudio,
  deleteLocalSessionAudio,
  deleteSessionAudio,
  sha256Hex,
} from "./attachments";

describe("attachment catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([0, 1, 1]);
    mocks.execute.mockResolvedValue([{ is_deleted: 1 }]);
    mocks.audioDelete.mockResolvedValue({ status: "ok", data: true });
    mocks.audioMetadata.mockResolvedValue({
      status: "ok",
      data: {
        filename: "audio.mp3",
        contentType: "audio/mpeg",
        sizeBytes: 84,
        sha256: "d".repeat(64),
      },
    });
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
    expect(statements).toHaveLength(3);
    expect(statements[1].sql).toContain("session.workspace_id");
    expect(statements[1].sql).toContain("session.deleted_at IS NULL");
    expect(statements[1].sql).not.toContain("/vault/");
    expect(statements[0].sql).toMatch(
      /WHEN session_attachments\.sha256 = \? THEN storage_kind/,
    );
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
    expect(statements[2].sql).toContain("attachment_local_state");
    expect(statements[2].sql).toContain("'present'");
    expect(statements[2].sql).toContain("ON CONFLICT(attachment_id)");
    expect(statements[2].params).toEqual([
      "session-1",
      "attachments/diagram 1.png",
    ]);
    expect(statements[2].expectedRowsAffected).toBe(1);
  });

  it("updates an existing physical attachment without creating a duplicate", async () => {
    mocks.executeTransaction.mockResolvedValue([1, 0, 1]);

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

    expect(mocks.executeTransaction.mock.calls[0]![0][0].params).toEqual([
      "diagram.png",
      "image/png",
      42,
      "b".repeat(64),
      "b".repeat(64),
      "b".repeat(64),
      "diagram.png",
      "session-1",
      "attachments/diagram.png",
    ]);
  });

  it("fails cataloging when local presence cannot be recorded", async () => {
    mocks.executeTransaction.mockResolvedValue([0, 1, 0]);

    await expect(
      catalogLocalNoteAttachment({
        sessionId: "session-1",
        attachmentId: "diagram.png",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "b".repeat(64),
      }),
    ).rejects.toThrow("attachment session is unavailable");
  });

  it("rejects missing or deleted sessions and unsafe attachment IDs", async () => {
    mocks.executeTransaction.mockResolvedValue([0, 0, 0]);
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

  it("catalogs primary audio with a stable logical identity and root-relative path", async () => {
    await catalogLocalSessionAudio("session-1");

    expect(mocks.audioMetadata).toHaveBeenCalledWith("session-1");
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(3);
    expect(statements[0].sql).toContain("WHEN session_attachments.sha256 = ?");
    expect(statements[0].sql).toContain("source_type = 'session_audio'");
    expect(statements[1].sql).toContain("session.workspace_id");
    expect(statements[1].params).toEqual([
      "session-audio:session-1",
      "audio.mp3",
      "audio.mp3",
      "audio/mpeg",
      84,
      "d".repeat(64),
      "session-1",
      "session-audio:session-1",
    ]);
    expect(statements[2].sql).toContain("attachment_local_state");
    expect(statements[2].sql).toContain("'present'");
    expect(statements[2].expectedRowsAffected).toBe(1);
  });

  it("updates the same audio row when the finalized format changes", async () => {
    mocks.executeTransaction.mockResolvedValue([1, 0, 1]);
    mocks.audioMetadata.mockResolvedValue({
      status: "ok",
      data: {
        filename: "audio.wav",
        contentType: "audio/wav",
        sizeBytes: 128,
        sha256: "e".repeat(64),
      },
    });

    await catalogLocalSessionAudio("session-1");

    const update = mocks.executeTransaction.mock.calls[0]![0][0];
    expect(update.params).toEqual([
      "audio.wav",
      "audio.wav",
      "audio/wav",
      128,
      "e".repeat(64),
      "e".repeat(64),
      "e".repeat(64),
      "session-audio:session-1",
      "session-1",
      "session-1",
    ]);
  });

  it("keeps canonical metadata when retention deletes only local audio bytes", async () => {
    await expect(
      deleteLocalSessionAudio("session-1", () => true),
    ).resolves.toBe(true);
    expect(mocks.audioDelete).toHaveBeenCalledWith("session-1");
    const localState = mocks.executeTransaction.mock.calls[0]![0][0];
    expect(localState.sql).toContain("attachment_local_state");
    expect(localState.params).toEqual([
      "session-audio:session-1",
      "session-1",
      "absent",
    ]);
    expect(localState.sql).not.toContain("UPDATE session_attachments");
  });

  it("tombstones logical audio before deleting local bytes", async () => {
    mocks.executeTransaction.mockResolvedValue([1]);

    await expect(deleteSessionAudio("session-1", () => true)).resolves.toBe(
      true,
    );

    expect(mocks.executeTransaction.mock.calls[0]![0][0].params).toEqual([
      "session-audio:session-1",
      "session-1",
    ]);
    expect(mocks.executeTransaction.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.audioDelete.mock.invocationCallOrder[0]!,
    );
    expect(mocks.audioDelete.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.executeTransaction.mock.invocationCallOrder[1]!,
    );
  });

  it("does not delete bytes when the logical tombstone fails", async () => {
    mocks.executeTransaction.mockRejectedValueOnce(
      new Error("database locked"),
    );

    await expect(deleteSessionAudio("session-1", () => true)).rejects.toThrow(
      "database locked",
    );
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  it("completes logical deletion when local audio is already absent", async () => {
    mocks.executeTransaction.mockResolvedValue([1]);
    mocks.audioDelete.mockResolvedValue({ status: "ok", data: false });

    await expect(deleteSessionAudio("session-1", () => true)).resolves.toBe(
      true,
    );
    expect(mocks.executeTransaction).toHaveBeenCalledTimes(2);
  });

  it("records local absence when retention finds no local audio", async () => {
    mocks.audioDelete.mockResolvedValue({ status: "ok", data: false });

    await expect(
      deleteLocalSessionAudio("session-1", () => true),
    ).resolves.toBe(false);
    expect(mocks.executeTransaction.mock.calls[0]![0][0].params).toEqual([
      "session-audio:session-1",
      "session-1",
      "absent",
    ]);
  });

  it("revalidates a logical tombstone before retrying file cleanup", async () => {
    await expect(
      cleanupDeletedSessionAudio("session-1", () => true),
    ).resolves.toBe(true);
    expect(mocks.execute).toHaveBeenCalledWith(
      expect.stringMatching(
        /deleted_at IS NOT NULL[\s\S]*attachment_local_state[\s\S]*availability = 'absent'/,
      ),
      ["session-audio:session-1", "session-1"],
    );
    expect(mocks.audioDelete).toHaveBeenCalledWith("session-1");

    vi.clearAllMocks();
    mocks.execute.mockResolvedValue([{ is_deleted: 0 }]);
    await expect(
      cleanupDeletedSessionAudio("session-1", () => true),
    ).resolves.toBe(false);
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  it("rechecks capture safety inside the serialized delete operation", async () => {
    await expect(deleteSessionAudio("session-1", () => false)).resolves.toBe(
      false,
    );
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });
});
