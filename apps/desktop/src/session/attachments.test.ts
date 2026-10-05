import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  audioDelete: vi.fn(),
  audioMetadata: vi.fn(),
  catalogNoteAttachment: vi.fn(),
  catalogSessionAudio: vi.fn(),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<unknown>) => write(),
  ),
  markSessionAudioAbsent: vi.fn(),
  markSessionAudioTranscriptionComplete: vi.fn(),
  tombstoneSessionAudio: vi.fn(),
}));

vi.mock("@anlg/plugin-session", () => ({
  commands: {
    catalogNoteAttachment: mocks.catalogNoteAttachment,
    catalogSessionAudio: mocks.catalogSessionAudio,
    markSessionAudioAbsent: mocks.markSessionAudioAbsent,
    markSessionAudioTranscriptionComplete:
      mocks.markSessionAudioTranscriptionComplete,
    tombstoneSessionAudio: mocks.tombstoneSessionAudio,
  },
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    audioDelete: mocks.audioDelete,
    audioMetadata: mocks.audioMetadata,
  },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

import {
  catalogLocalNoteAttachment,
  catalogLocalSessionAudio,
  deleteSessionAudio,
  markSessionAudioTranscriptionComplete,
  sha256Hex,
} from "./attachments";

describe("attachment catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.catalogNoteAttachment.mockResolvedValue({ status: "ok", data: null });
    mocks.catalogSessionAudio.mockResolvedValue({ status: "ok", data: null });
    mocks.markSessionAudioAbsent.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.markSessionAudioTranscriptionComplete.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.tombstoneSessionAudio.mockResolvedValue({ status: "ok", data: null });
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

  it("rejects unsafe attachment metadata before cataloging", async () => {
    await expect(
      catalogLocalNoteAttachment({
        sessionId: "session-1",
        attachmentId: "../diagram.png",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "a".repeat(64),
      }),
    ).rejects.toThrow("attachment ID");

    await expect(
      catalogLocalNoteAttachment({
        sessionId: "session-1",
        attachmentId: "diagram.png",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: -1,
        sha256: "a".repeat(64),
      }),
    ).rejects.toThrow("invalid attachment size");
    expect(mocks.catalogNoteAttachment).not.toHaveBeenCalled();
  });

  it("preserves errors from session-audio metadata lookup", async () => {
    mocks.audioMetadata.mockResolvedValue({
      status: "error",
      error: "audio metadata unavailable",
    });

    await expect(catalogLocalSessionAudio("session-1")).rejects.toThrow(
      "audio metadata unavailable",
    );
    expect(mocks.catalogSessionAudio).not.toHaveBeenCalled();
  });

  it("computes a stable lowercase SHA-256 checksum", async () => {
    const bytes = new TextEncoder().encode("hello").buffer;

    await expect(sha256Hex(bytes)).resolves.toBe(
      "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    );
  });

  it("does not delete bytes when the logical tombstone fails", async () => {
    mocks.tombstoneSessionAudio.mockResolvedValue({
      status: "error",
      error: "database locked",
    });

    await expect(deleteSessionAudio("session-1", () => true)).rejects.toThrow(
      "database locked",
    );
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  it("does not mark audio absent when removing local bytes fails", async () => {
    mocks.audioDelete.mockResolvedValue({
      status: "error",
      error: "audio delete failed",
    });

    await expect(deleteSessionAudio("session-1", () => true)).rejects.toThrow(
      "audio delete failed",
    );
    expect(mocks.markSessionAudioAbsent).not.toHaveBeenCalled();
  });

  it("rechecks capture safety inside the serialized delete operation", async () => {
    await expect(deleteSessionAudio("session-1", () => false)).resolves.toBe(
      false,
    );
    expect(mocks.tombstoneSessionAudio).not.toHaveBeenCalled();
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  it("propagates transcription completion command errors", async () => {
    mocks.markSessionAudioTranscriptionComplete.mockResolvedValue({
      status: "error",
      error: "database unavailable",
    });

    await expect(
      markSessionAudioTranscriptionComplete("session-1"),
    ).rejects.toThrow("database unavailable");
  });
});
