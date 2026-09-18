import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  cleanupDeletedSessionAudio: vi.fn(),
  deleteLocalSessionAudio: vi.fn(),
  execute: vi.fn(),
  getSessionMode: vi.fn(),
  live: { loading: false, sessionId: null as string | null },
}));

vi.mock("~/session/attachments", () => ({
  cleanupDeletedSessionAudio: mocks.cleanupDeletedSessionAudio,
  deleteLocalSessionAudio: mocks.deleteLocalSessionAudio,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({
      getSessionMode: mocks.getSessionMode,
      live: mocks.live,
    }),
  },
}));

import {
  cleanupExpiredAudio,
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
  sessionAudioExpired,
  subscribeToSessionAudioRetention,
} from "./audio-retention";

function mockCleanupRows(
  sessions: Array<{
    id: string;
    created_at: string;
    has_words: number;
    transcript_processing?: number;
  }>,
  logicallyDeleted: Array<{ session_id: string }> = [],
) {
  mocks.execute.mockImplementation((sql: string) =>
    Promise.resolve(
      sql.includes("SELECT DISTINCT attachment.session_id")
        ? logicallyDeleted
        : sessions,
    ),
  );
}

describe("audio retention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.cleanupDeletedSessionAudio.mockResolvedValue(true);
    mocks.deleteLocalSessionAudio.mockResolvedValue(true);
    mocks.getSessionMode.mockReturnValue("inactive");
    mocks.live.loading = false;
    mocks.live.sessionId = null;
    mockCleanupRows([]);
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
    mockCleanupRows([
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

    expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledTimes(1);
    expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledWith(
      "expired",
      expect.any(Function),
    );
    expect(deleted).toEqual(["expired"]);
  });

  test("retention none deletes idle audio even when no transcript was saved", async () => {
    mockCleanupRows([
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
    ).resolves.toEqual(["unprocessed", "processed"]);
    expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledWith(
      "processed",
      expect.any(Function),
    );
  });

  test.each(["none", "oneWeek"] as const)(
    "unfinished transcription does not extend %s audio retention",
    async (policy) => {
      mockCleanupRows([
        {
          id: "partial",
          created_at: "2026-05-01T00:00:00.000Z",
          has_words: 1,
          transcript_processing: 1,
        },
      ]);

      await expect(
        cleanupExpiredAudio(policy, Date.parse("2026-05-13T00:00:00.000Z")),
      ).resolves.toEqual(["partial"]);
      expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledWith(
        "partial",
        expect.any(Function),
      );
    },
  );

  test("deletes processed audio immediately when retention is none", async () => {
    const listener = vi.fn();
    const unsubscribe = subscribeToSessionAudioRetention(listener);

    await expect(
      deleteProcessedAudioForRetention("none", "processed"),
    ).resolves.toBe(true);
    expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledWith(
      "processed",
      expect.any(Function),
    );
    expect(listener).toHaveBeenNthCalledWith(1, {
      phase: "deleting",
      sessionId: "processed",
    });
    expect(listener).toHaveBeenNthCalledWith(2, {
      phase: "deleted",
      sessionId: "processed",
    });
    unsubscribe();
  });

  test("deletes idle unprocessed audio when retention is none", async () => {
    await expect(
      deleteProcessedAudioForRetention("none", "unprocessed"),
    ).resolves.toBe(true);
    expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledOnce();
  });

  test("deletes idle partially persisted audio when retention is none", async () => {
    await expect(
      deleteProcessedAudioForRetention("none", "partial"),
    ).resolves.toBe(true);
    expect(mocks.deleteLocalSessionAudio).toHaveBeenCalledOnce();
  });

  test("skips immediate deletion for retained audio", async () => {
    await expect(
      deleteProcessedAudioForRetention("oneDay", "processed"),
    ).resolves.toBe(false);
    expect(mocks.execute).not.toHaveBeenCalled();
    expect(mocks.deleteLocalSessionAudio).not.toHaveBeenCalled();
  });

  test("only scans logical deletions when retention is forever", async () => {
    await expect(cleanupExpiredAudio("forever")).resolves.toEqual([]);
    expect(mocks.execute).toHaveBeenCalledTimes(1);
    expect(mocks.deleteLocalSessionAudio).not.toHaveBeenCalled();
  });

  test("does not report failed audio deletions as deleted", async () => {
    mockCleanupRows([
      {
        id: "expired",
        created_at: "2026-05-01T00:00:00.000Z",
        has_words: 1,
      },
    ]);
    mocks.deleteLocalSessionAudio.mockRejectedValueOnce(
      new Error("disk failure"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    await expect(
      cleanupExpiredAudio("oneDay", Date.parse("2026-05-13T00:00:00.000Z")),
    ).resolves.toEqual([]);
    expect(consoleError).toHaveBeenCalled();
    consoleError.mockRestore();
  });

  test("does not report a device without local audio as deleted", async () => {
    mockCleanupRows([
      {
        id: "remote-only",
        created_at: "2026-05-01T00:00:00.000Z",
        has_words: 1,
      },
    ]);
    mocks.deleteLocalSessionAudio.mockResolvedValueOnce(false);

    await expect(
      cleanupExpiredAudio("oneDay", Date.parse("2026-05-13T00:00:00.000Z")),
    ).resolves.toEqual([]);
  });

  test("retries local cleanup for logically deleted audio", async () => {
    mockCleanupRows([], [{ session_id: "deleted-session" }]);

    await expect(cleanupExpiredAudio("forever")).resolves.toEqual([
      "deleted-session",
    ]);
    expect(mocks.cleanupDeletedSessionAudio).toHaveBeenCalledWith(
      "deleted-session",
      expect.any(Function),
    );
    expect(mocks.execute.mock.calls[0]![0]).toContain(
      "COALESCE(local.availability, 'present') != 'absent'",
    );
  });

  test("does not clean a remote tombstone while the session is recording", async () => {
    mockCleanupRows([], [{ session_id: "active-session" }]);
    mocks.getSessionMode.mockReturnValue("active");

    await expect(cleanupExpiredAudio("forever")).resolves.toEqual([]);
    expect(mocks.cleanupDeletedSessionAudio).not.toHaveBeenCalled();
  });

  test("does not clean audio while capture startup is loading", async () => {
    mockCleanupRows([], [{ session_id: "starting-session" }]);
    mocks.live.sessionId = "starting-session";
    mocks.live.loading = true;

    await expect(cleanupExpiredAudio("forever")).resolves.toEqual([]);
    expect(mocks.cleanupDeletedSessionAudio).not.toHaveBeenCalled();
  });
});
