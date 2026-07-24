import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createCloudsyncActivityReleaseManager } from "./cloudsync-activity";

describe("CloudSync activity release", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("hands off after bounded attempts and retries until release succeeds", async () => {
    const end = vi
      .fn()
      .mockRejectedValueOnce(new Error("failure 1"))
      .mockRejectedValueOnce(new Error("failure 2"))
      .mockRejectedValueOnce(new Error("failure 3"))
      .mockRejectedValueOnce(new Error("failure 4"))
      .mockResolvedValueOnce(undefined);
    const onDeferred = vi.fn();
    const manager = createCloudsyncActivityReleaseManager({
      end,
      foregroundRetryDelaysMs: [10, 20],
      backgroundRetryIntervalMs: 100,
      onDeferred,
    });

    const handoff = manager.release("capture", "lease-1");
    await vi.advanceTimersByTimeAsync(30);
    await handoff;

    expect(end).toHaveBeenCalledTimes(3);
    expect(onDeferred).toHaveBeenCalledOnce();

    await vi.advanceTimersByTimeAsync(100);
    expect(end).toHaveBeenCalledTimes(4);

    await vi.advanceTimersByTimeAsync(100);
    expect(end).toHaveBeenCalledTimes(5);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(end).toHaveBeenCalledTimes(5);
  });

  it("deduplicates foreground and background retries for one native lease", async () => {
    const end = vi
      .fn()
      .mockRejectedValueOnce(new Error("failure 1"))
      .mockRejectedValueOnce(new Error("failure 2"))
      .mockResolvedValueOnce(undefined);
    const manager = createCloudsyncActivityReleaseManager({
      end,
      foregroundRetryDelaysMs: [],
      backgroundRetryIntervalMs: 100,
      onDeferred: vi.fn(),
    });

    const first = manager.release("enhance", "lease-1");
    const duplicate = manager.release("enhance", "lease-1");

    expect(duplicate).toBe(first);
    await first;
    expect(end).toHaveBeenCalledOnce();

    const pendingDuplicate = manager.release("enhance", "lease-1");
    expect(pendingDuplicate).toBe(first);

    await vi.advanceTimersByTimeAsync(100);
    expect(end).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(100);
    expect(end).toHaveBeenCalledTimes(3);
  });
});
