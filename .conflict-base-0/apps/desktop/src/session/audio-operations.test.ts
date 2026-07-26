import { describe, expect, it, vi } from "vitest";

import { enqueueSessionAudioOperation } from "./audio-operations";

describe("session audio operations", () => {
  it("serializes operations for the same session after failures", async () => {
    let releaseFirst: (() => void) | undefined;
    const calls: string[] = [];
    const first = enqueueSessionAudioOperation("session-1", async () => {
      calls.push("first:start");
      await new Promise<void>((resolve) => {
        releaseFirst = resolve;
      });
      calls.push("first:end");
      throw new Error("failed");
    });
    const secondOperation = vi.fn(async () => {
      calls.push("second");
    });
    const second = enqueueSessionAudioOperation("session-1", secondOperation);

    await vi.waitFor(() => expect(releaseFirst).toBeTypeOf("function"));
    expect(secondOperation).not.toHaveBeenCalled();
    releaseFirst?.();

    await expect(first).rejects.toThrow("failed");
    await expect(second).resolves.toBeUndefined();
    expect(calls).toEqual(["first:start", "first:end", "second"]);
  });

  it("does not block a different session", async () => {
    let releaseFirst: (() => void) | undefined;
    const first = enqueueSessionAudioOperation(
      "session-1",
      () =>
        new Promise<void>((resolve) => {
          releaseFirst = resolve;
        }),
    );

    await expect(
      enqueueSessionAudioOperation("session-2", async () => "ready"),
    ).resolves.toBe("ready");
    releaseFirst?.();
    await first;
  });
});
