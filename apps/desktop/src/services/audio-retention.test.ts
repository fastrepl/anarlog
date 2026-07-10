import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  audioDelete: vi.fn(),
  execute: vi.fn(),
  getSessionMode: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: { audioDelete: mocks.audioDelete },
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({ getSessionMode: mocks.getSessionMode }),
  },
}));

import {
  cleanupExpiredAudio,
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
  sessionAudioExpired,
} from "./audio-retention";

describe("audio retention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.audioDelete.mockResolvedValue({ status: "ok", data: null });
    mocks.getSessionMode.mockReturnValue("inactive");
  });

  test("normalizes current and legacy values", () => {
    expect(normalizeAudioRetention("none")).toBe("none");
    expect(normalizeAudioRetention("oneWeek")).toBe("oneWeek");
    expect(normalizeAudioRetention("forever")).toBe("forever");
    expect(normalizeAudioRetention(false)).toBe("none");
    expect(normalizeAudioRetention(true)).toBe("forever");
    expect(normalizeAudioRetention("invalid")).toBe("forever");
    expect(normalizeAudioRetention("invalid", undefined)).toBeUndefined();
  });

  test("applies each retention window", () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");

    expect(sessionAudioExpired("not-a-date", "none", now)).toBe(true);
    expect(
      sessionAudioExpired("2026-01-01T00:00:00.000Z", "forever", now),
    ).toBe(false);
    expect(sessionAudioExpired("2026-05-11T23:59:59.999Z", "oneDay", now)).toBe(
      true,
    );
    expect(sessionAudioExpired("2026-05-12T00:00:00.001Z", "oneDay", now)).toBe(
      false,
    );
    expect(sessionAudioExpired("not-a-date", "oneDay", now)).toBe(false);
  });

  test("deletes only expired inactive SQLite sessions", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "expired",
        created_at: "2026-05-11T23:59:59.999Z",
        has_words: 1,
      },
      {
        id: "fresh",
        created_at: "2026-05-12T00:00:00.001Z",
        has_words: 1,
      },
      {
        id: "active",
        created_at: "2026-05-11T23:59:59.999Z",
        has_words: 1,
      },
    ]);
    mocks.getSessionMode.mockImplementation((sessionId) =>
      sessionId === "active" ? "active" : "inactive",
    );

    const deleted = await cleanupExpiredAudio(
      "oneDay",
      Date.parse("2026-05-13T00:00:00.000Z"),
    );

    expect(mocks.audioDelete).toHaveBeenCalledTimes(1);
    expect(mocks.audioDelete).toHaveBeenCalledWith("expired");
    expect(deleted).toEqual(["expired"]);
  });

  test("retention none keeps audio until transcript words exist", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "unprocessed",
        created_at: "2026-05-13T00:00:00.000Z",
        has_words: 0,
      },
      {
        id: "processed",
        created_at: "2026-05-13T00:00:00.000Z",
        has_words: 1,
      },
    ]);

    await expect(
      cleanupExpiredAudio("none", Date.parse("2026-05-13T00:00:00.000Z")),
    ).resolves.toEqual(["processed"]);
    expect(mocks.audioDelete).toHaveBeenCalledWith("processed");
  });

  test("deletes processed audio immediately when retention is none", async () => {
    mocks.execute.mockResolvedValueOnce([{ has_words: 1 }]);

    await expect(
      deleteProcessedAudioForRetention("none", "processed"),
    ).resolves.toBe(true);
    expect(mocks.audioDelete).toHaveBeenCalledWith("processed");
  });

  test("keeps unprocessed audio when retention is none", async () => {
    mocks.execute.mockResolvedValueOnce([{ has_words: 0 }]);

    await expect(
      deleteProcessedAudioForRetention("none", "unprocessed"),
    ).resolves.toBe(false);
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  test("skips immediate deletion for retained audio", async () => {
    await expect(
      deleteProcessedAudioForRetention("oneDay", "processed"),
    ).resolves.toBe(false);
    expect(mocks.execute).not.toHaveBeenCalled();
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  test("does not scan SQLite when retention is forever", async () => {
    await expect(cleanupExpiredAudio("forever")).resolves.toEqual([]);
    expect(mocks.execute).not.toHaveBeenCalled();
    expect(mocks.audioDelete).not.toHaveBeenCalled();
  });

  test("does not report failed audio deletions as deleted", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "expired",
        created_at: "2026-05-01T00:00:00.000Z",
        has_words: 1,
      },
    ]);
    mocks.audioDelete.mockResolvedValueOnce({
      status: "error",
      error: "disk failure",
    });
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    await expect(
      cleanupExpiredAudio("oneDay", Date.parse("2026-05-13T00:00:00.000Z")),
    ).resolves.toEqual([]);
    expect(consoleError).toHaveBeenCalled();
    consoleError.mockRestore();
  });
});
