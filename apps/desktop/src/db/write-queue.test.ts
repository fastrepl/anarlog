import { describe, expect, it, vi } from "vitest";

import { enqueueDatabaseWrite, flushDatabaseWrites } from "./write-queue";

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

  it("returns a serialized write's result", async () => {
    await expect(
      enqueueDatabaseWrite("human:1", async () => "human-1"),
    ).resolves.toBe("human-1");
  });
});
