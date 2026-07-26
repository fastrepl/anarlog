import { describe, expect, it, vi } from "vitest";

import {
  enqueueDatabaseWrite,
  flushDatabaseWrites,
  flushDatabaseWritesByPrefix,
  flushDatabaseWritesWithin,
} from "./write-queue";

describe("database write queue", () => {
  it("serializes writes for the same record", async () => {
    let releaseFirst: (() => void) | undefined;
    const order: string[] = [];
    const first = enqueueDatabaseWrite(
      "session:1",
      () =>
        new Promise<void>((resolve) => {
          order.push("first-start");
          releaseFirst = () => {
            order.push("first-end");
            resolve();
          };
        }),
    );
    const second = enqueueDatabaseWrite("session:1", async () => {
      order.push("second");
    });

    await vi.waitFor(() => expect(releaseFirst).toBeTypeOf("function"));
    expect(order).toEqual(["first-start"]);
    releaseFirst?.();
    await Promise.all([first, second]);
    expect(order).toEqual(["first-start", "first-end", "second"]);
  });

  it("waits for pending writes before save completes", async () => {
    let release: (() => void) | undefined;
    void enqueueDatabaseWrite(
      "session:2",
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );

    const flushed = vi.fn();
    const flush = flushDatabaseWrites().then(flushed);
    await vi.waitFor(() => expect(release).toBeTypeOf("function"));
    expect(flushed).not.toHaveBeenCalled();
    release?.();
    await flush;
    expect(flushed).toHaveBeenCalledOnce();
  });

  it("flushes selected writes without waiting for unrelated work", async () => {
    let releaseSession: (() => void) | undefined;
    let releaseBackground: (() => void) | undefined;
    const sessionWrite = enqueueDatabaseWrite(
      "session:2",
      () =>
        new Promise<void>((resolve) => {
          releaseSession = resolve;
        }),
    );
    const backgroundWrite = enqueueDatabaseWrite(
      "background-sync",
      () =>
        new Promise<void>((resolve) => {
          releaseBackground = resolve;
        }),
    );

    const flushed = vi.fn();
    const flush = flushDatabaseWrites(["session:2"]).then(flushed);
    await vi.waitFor(() => {
      expect(releaseSession).toBeTypeOf("function");
      expect(releaseBackground).toBeTypeOf("function");
    });
    expect(flushed).not.toHaveBeenCalled();

    releaseSession?.();
    await Promise.all([sessionWrite, flush]);
    expect(flushed).toHaveBeenCalledOnce();

    releaseBackground?.();
    await backgroundWrite;
  });

  it("drains a newer keyed write before reporting an older failure", async () => {
    let rejectFirst: ((error: Error) => void) | undefined;
    let releaseSecond: (() => void) | undefined;
    const first = enqueueDatabaseWrite(
      "session:failed",
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectFirst = reject;
        }),
    );
    const flush = flushDatabaseWrites(["session:failed"]);
    const second = enqueueDatabaseWrite(
      "session:failed",
      () =>
        new Promise<void>((resolve) => {
          releaseSecond = resolve;
        }),
    );

    await vi.waitFor(() => expect(rejectFirst).toBeTypeOf("function"));
    rejectFirst?.(new Error("first write failed"));
    await vi.waitFor(() => expect(releaseSecond).toBeTypeOf("function"));

    let flushSettled = false;
    void flush.then(
      () => {
        flushSettled = true;
      },
      () => {
        flushSettled = true;
      },
    );
    await Promise.resolve();
    expect(flushSettled).toBe(false);

    releaseSecond?.();
    await expect(first).rejects.toThrow("first write failed");
    await second;
    await expect(flush).rejects.toThrow("first write failed");
  });

  it("drains writes added to an earlier key while another key is pending", async () => {
    let releaseFirst: (() => void) | undefined;
    let releaseSecond: (() => void) | undefined;
    let releaseLateFirst: (() => void) | undefined;
    const first = enqueueDatabaseWrite(
      "session:first",
      () =>
        new Promise<void>((resolve) => {
          releaseFirst = resolve;
        }),
    );
    const second = enqueueDatabaseWrite(
      "session:second",
      () =>
        new Promise<void>((resolve) => {
          releaseSecond = resolve;
        }),
    );
    const flushed = vi.fn();
    const flush = flushDatabaseWrites(["session:first", "session:second"]).then(
      flushed,
    );

    await vi.waitFor(() => {
      expect(releaseFirst).toBeTypeOf("function");
      expect(releaseSecond).toBeTypeOf("function");
    });
    releaseFirst?.();
    await first;
    await Promise.resolve();
    const lateFirst = enqueueDatabaseWrite(
      "session:first",
      () =>
        new Promise<void>((resolve) => {
          releaseLateFirst = resolve;
        }),
    );
    await vi.waitFor(() => expect(releaseLateFirst).toBeTypeOf("function"));

    releaseSecond?.();
    await second;
    expect(flushed).not.toHaveBeenCalled();

    releaseLateFirst?.();
    await Promise.all([lateFirst, flush]);
    expect(flushed).toHaveBeenCalledOnce();
  });

  it("returns a serialized write's result", async () => {
    await expect(
      enqueueDatabaseWrite("human:1", async () => "human-1"),
    ).resolves.toBe("human-1");
  });

  it("flushes a write domain without waiting for unrelated keys", async () => {
    let releaseCache: (() => void) | undefined;
    let releaseBackground: (() => void) | undefined;
    const cacheWrite = enqueueDatabaseWrite(
      "shared-note-cache:viewer-1",
      () =>
        new Promise<void>((resolve) => {
          releaseCache = resolve;
        }),
    );
    const backgroundWrite = enqueueDatabaseWrite(
      "background-sync",
      () =>
        new Promise<void>((resolve) => {
          releaseBackground = resolve;
        }),
    );

    const flushed = vi.fn();
    const flush = flushDatabaseWritesByPrefix(["shared-note-cache:"]).then(
      flushed,
    );
    await vi.waitFor(() => {
      expect(releaseCache).toBeTypeOf("function");
      expect(releaseBackground).toBeTypeOf("function");
    });

    releaseCache?.();
    await Promise.all([cacheWrite, flush]);
    expect(flushed).toHaveBeenCalledOnce();

    releaseBackground?.();
    await backgroundWrite;
  });

  it("drains sibling prefix writes before reporting a failure", async () => {
    let rejectFirst: ((error: Error) => void) | undefined;
    let releaseSibling: (() => void) | undefined;
    let releaseLate: (() => void) | undefined;
    const first = enqueueDatabaseWrite(
      "shared-note-cache:first",
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectFirst = reject;
        }),
    );
    const sibling = enqueueDatabaseWrite(
      "shared-note-cache:sibling",
      () =>
        new Promise<void>((resolve) => {
          releaseSibling = resolve;
        }),
    );
    const flush = flushDatabaseWritesByPrefix(["shared-note-cache:"]);

    await vi.waitFor(() => {
      expect(rejectFirst).toBeTypeOf("function");
      expect(releaseSibling).toBeTypeOf("function");
    });
    rejectFirst?.(new Error("first write failed"));
    await expect(first).rejects.toThrow("first write failed");
    await Promise.resolve();
    const late = enqueueDatabaseWrite(
      "shared-note-cache:first",
      () =>
        new Promise<void>((resolve) => {
          releaseLate = resolve;
        }),
    );
    await vi.waitFor(() => expect(releaseLate).toBeTypeOf("function"));

    releaseSibling?.();
    await sibling;
    let flushSettled = false;
    void flush.then(
      () => {
        flushSettled = true;
      },
      () => {
        flushSettled = true;
      },
    );
    await Promise.resolve();
    expect(flushSettled).toBe(false);

    releaseLate?.();
    await late;
    await expect(flush).rejects.toThrow("first write failed");
  });

  it("bounds lifecycle flushes without discarding the pending write", async () => {
    let rejectWrite: ((error: Error) => void) | undefined;
    const write = enqueueDatabaseWrite(
      "session:slow",
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectWrite = reject;
        }),
    );
    await vi.waitFor(() => expect(rejectWrite).toBeTypeOf("function"));
    vi.useFakeTimers();

    const flush = flushDatabaseWritesWithin(5000);
    const rejection = expect(flush).rejects.toThrow(
      "Timed out waiting for local database writes",
    );
    await vi.advanceTimersByTimeAsync(5000);
    await rejection;

    vi.useRealTimers();
    rejectWrite?.(new Error("late write failure"));
    await expect(write).rejects.toThrow("late write failure");
  });
});
