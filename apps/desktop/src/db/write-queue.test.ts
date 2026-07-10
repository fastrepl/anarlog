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

  it("returns a serialized write's result", async () => {
    await expect(
      enqueueDatabaseWrite("human:1", async () => "human-1"),
    ).resolves.toBe("human-1");
  });
});
