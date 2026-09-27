import { describe, expect, it, vi } from "vitest";

import { createCaptureAudioRecovery } from "./capture-audio-recovery";

function setup() {
  let now = 90_000;
  const chunk = {
    id: "0-0-60000.mp3",
    path: "/chunk.mp3",
    capture_started_at: 0,
    start_ms: 0,
    audio_start_ms: 0,
    end_ms: 60_000,
  };
  const list = vi.fn(async () => [chunk]);
  const acknowledge = vi.fn(async (_chunk: { id: string }) => {});
  const flush = vi.fn(async () => {});
  const repair = vi.fn(async (_chunk, _gaps, _signal: AbortSignal) => {});
  const worker = createCaptureAudioRecovery({
    startedAt: 0,
    list,
    acknowledge,
    flush,
    repair,
    now: () => now,
  });
  return {
    worker,
    list,
    acknowledge,
    flush,
    repair,
    setNow: (value: number) => {
      now = value;
    },
  };
}

describe("capture audio recovery", () => {
  it("releases healthy audio only after transcript persistence", async () => {
    const { worker, flush, acknowledge, repair } = setup();
    worker.persistedThrough(70_000);
    await worker.tick();
    expect(flush).toHaveBeenCalledOnce();
    expect(repair).not.toHaveBeenCalled();
    expect(acknowledge.mock.invocationCallOrder[0]).toBeGreaterThan(
      flush.mock.invocationCallOrder[0]!,
    );
  });

  it("keeps outage audio and repairs only the affected interval after reconnection", async () => {
    const { worker, repair, acknowledge, setNow } = setup();
    worker.persistedThrough(20_000);
    setNow(30_000);
    worker.interrupted();
    setNow(90_000);
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    worker.connected();
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 19_000, end: 60_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("does not acknowledge a repair whose database write failed", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.persistenceFailed();
    repair.mockRejectedValueOnce(new Error("database or disk is full"));
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    expect((await worker.stop(false)).incomplete).toBe(true);
  });

  it("runs only one batch job while new ticks arrive", async () => {
    const { worker, repair } = setup();
    let finish!: () => void;
    repair.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    worker.persistenceFailed();
    const first = worker.tick();
    await vi.waitFor(() => expect(repair).toHaveBeenCalledOnce());
    const second = worker.tick();
    expect(first).toBe(second);
    finish();
    await first;
    expect(repair).toHaveBeenCalledOnce();
  });

  it("aborts repair at a zero-retention stop and never acknowledges its audio", async () => {
    const { worker, repair, acknowledge } = setup();
    repair.mockImplementation(
      (_chunk, _gaps, signal) =>
        new Promise<void>((_resolve, reject) => {
          signal.addEventListener("abort", () => reject(signal.reason), {
            once: true,
          });
        }),
    );
    worker.persistenceFailed();
    void worker.tick();
    await vi.waitFor(() => expect(repair).toHaveBeenCalledOnce());
    expect((await worker.stop(false)).incomplete).toBe(true);
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("processes batch-only capture in bounded chunks during the meeting", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.batchOnly();
    await worker.tick();
    expect(repair).toHaveBeenCalledOnce();
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("recovers only the backlog when reattaching to an active meeting", async () => {
    const { worker, list, repair, acknowledge, setNow } = setup();
    worker.recoverPending();
    await worker.tick();
    const chunk = list.mock.results[0]!.value;
    const [first] = await chunk;
    list.mockResolvedValue([{ ...first!, start_ms: 120_000, end_ms: 180_000 }]);
    setNow(200_000);
    worker.persistedThrough(190_000);
    await worker.tick();
    expect(repair).toHaveBeenCalledOnce();
    expect(acknowledge).toHaveBeenCalledTimes(2);
  });

  it("keeps a partially repaired chunk until the remaining live text is durable", async () => {
    const { worker, repair, acknowledge, setNow } = setup();
    setNow(30_000);
    worker.recoverPending();
    setNow(90_000);
    worker.persistedThrough(40_000);
    await worker.tick();
    expect(repair).not.toHaveBeenCalled();
    expect(acknowledge).not.toHaveBeenCalled();
    worker.persistedThrough(70_000);
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 0, end: 30_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("does not acknowledge audio when another incident arrives during repair", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.recoverPending();
    repair.mockImplementation(async () => worker.interrupted());
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    expect((await worker.stop(false)).incomplete).toBe(true);
  });
});

it.each([129, 256])(
  "drains all %s retained chunks when stopping",
  async (count) => {
    const { worker, list, acknowledge, repair, setNow } = setup();
    let chunks = Array.from({ length: count }, (_, index) => ({
      id: `${index}.mp3`,
      path: `/chunk-${index}.mp3`,
      capture_started_at: 0,
      start_ms: index * 60_000,
      audio_start_ms: index * 60_000,
      end_ms: (index + 1) * 60_000,
    }));
    list.mockImplementation(async () => chunks.slice(0, 128));
    acknowledge.mockImplementation(async (chunk) => {
      chunks = chunks.filter((candidate) => candidate.id !== chunk.id);
    });
    setNow(count * 60_000);
    worker.recoverPending();
    expect(await worker.stop(true)).toEqual({ incomplete: false });
    expect(repair).toHaveBeenCalledTimes(count);
    expect(chunks).toEqual([]);
  },
);

it("stops draining when offline instead of spinning on the same page", async () => {
  const { worker, list, acknowledge } = setup();
  const chunk = (await list())[0]!;
  list.mockResolvedValue(Array.from({ length: 128 }, () => chunk));
  list.mockClear();
  worker.interrupted();
  expect(await worker.stop(true)).toEqual({ incomplete: true });
  expect(list).toHaveBeenCalledOnce();
  expect(acknowledge).not.toHaveBeenCalled();
});
