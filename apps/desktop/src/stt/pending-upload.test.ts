import { afterEach, describe, expect, it, vi } from "vitest";

import {
  clearPendingUpload,
  consumePendingUpload,
  reservePendingUpload,
  setPendingUpload,
} from "./pending-upload";

describe("pending uploads", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("expires an abandoned upload", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-04T00:00:00.000Z"));
    setPendingUpload("expired-session", {
      kind: "audio",
      filePath: "/tmp/audio.mp3",
    });

    vi.advanceTimersByTime(30 * 60 * 1_000);

    expect(consumePendingUpload("expired-session")).toBeNull();
  });

  it("does not evict an active handoff when more than 32 uploads are pending", () => {
    for (let index = 0; index < 33; index += 1) {
      setPendingUpload(`session-${index}`, {
        kind: "transcript",
        filePath: `/tmp/transcript-${index}.txt`,
      });
    }

    expect(consumePendingUpload("session-0")).toEqual({
      kind: "transcript",
      filePath: "/tmp/transcript-0.txt",
    });
    expect(consumePendingUpload("session-32")).toEqual({
      kind: "transcript",
      filePath: "/tmp/transcript-32.txt",
    });
    for (let index = 1; index < 32; index += 1) {
      clearPendingUpload(`session-${index}`);
    }
  });

  it("rejects new handoffs at capacity without evicting existing ones", () => {
    const retainedSessionIds: string[] = [];
    for (let index = 0; index < 64; index += 1) {
      const sessionId = `capacity-session-${index}`;
      retainedSessionIds.push(sessionId);
      expect(
        setPendingUpload(sessionId, {
          kind: "audio",
          filePath: `/tmp/audio-${index}.mp3`,
        }),
      ).toBe(true);
    }

    expect(
      setPendingUpload("rejected-session", {
        kind: "audio",
        filePath: "/tmp/rejected.mp3",
      }),
    ).toBe(false);
    expect(consumePendingUpload("capacity-session-0")).toEqual({
      kind: "audio",
      filePath: "/tmp/audio-0.mp3",
    });
    expect(consumePendingUpload("capacity-session-63")).toEqual({
      kind: "audio",
      filePath: "/tmp/audio-63.mp3",
    });

    for (const sessionId of retainedSessionIds.slice(1, -1)) {
      clearPendingUpload(sessionId);
    }
  });

  it("reserves capacity before a session is created", () => {
    const reservations = Array.from({ length: 64 }, (_, index) =>
      reservePendingUpload({
        kind: "transcript",
        filePath: `/tmp/transcript-${index}.txt`,
      }),
    );

    expect(reservations.every(Boolean)).toBe(true);
    expect(
      reservePendingUpload({
        kind: "transcript",
        filePath: "/tmp/rejected.txt",
      }),
    ).toBeNull();

    reservations.forEach((reservation) => reservation?.cancel());
  });

  it("keeps a replacement handoff when the prior expiration fires", () => {
    vi.useFakeTimers();
    setPendingUpload("session-1", {
      kind: "audio",
      filePath: "/tmp/old.mp3",
    });
    vi.advanceTimersByTime(20 * 60 * 1_000);
    setPendingUpload("session-1", {
      kind: "audio",
      filePath: "/tmp/new.mp3",
    });

    vi.advanceTimersByTime(10 * 60 * 1_000);

    expect(consumePendingUpload("session-1")).toEqual({
      kind: "audio",
      filePath: "/tmp/new.mp3",
    });
  });
});
