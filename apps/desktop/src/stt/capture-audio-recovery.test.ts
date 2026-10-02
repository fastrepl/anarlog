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

  it("restores and repairs only the persisted outage interval", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.restore({
      gaps: [{ start: 20_000, end: 50_000 }],
      awaitingConnection: false,
      storageFailed: false,
      confirmedThrough: 70_000,
    });

    await worker.tick();

    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 20_000, end: 50_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("restores an open gap after a newer connection event", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.connected();
    worker.restore({
      gaps: [],
      openGapStart: 30_000,
      awaitingConnection: true,
      storageFailed: false,
      confirmedThrough: 70_000,
    });

    await worker.tick();

    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 30_000, end: 60_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("repairs a whole earlier capture chunk before acknowledging it", async () => {
    const earlier = {
      id: "-100000-0-60000-0.mp3",
      path: "/earlier.mp3",
      capture_started_at: -100_000,
      start_ms: 0,
      audio_start_ms: 0,
      end_ms: 60_000,
    };
    let chunks = [earlier];
    const acknowledge = vi.fn(async (_chunk: { id: string }) => {
      chunks = [];
    });
    const repair = vi.fn(async (_chunk, _gaps, _signal: AbortSignal) => {});
    const worker = createCaptureAudioRecovery({
      startedAt: 0,
      list: async () => chunks,
      acknowledge,
      flush: async () => {},
      repair,
      inherited: (chunk) => chunk.capture_started_at < 0,
      now: () => 1_000,
    });
    worker.connected();
    await worker.tick();
    expect(repair).toHaveBeenCalledWith(earlier, [], expect.anything());
    expect(acknowledge).toHaveBeenCalledWith(earlier);
    expect((await worker.stop()).incomplete).toBe(false);
  });

  it("does not acknowledge a repair whose database write failed", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.persistenceFailed();
    repair.mockRejectedValueOnce(new Error("database or disk is full"));
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    expect((await worker.stop()).incomplete).toBe(true);
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

  it("keeps unrepaired audio when repair fails at stop", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.persistenceFailed();
    repair.mockRejectedValue(new Error("offline"));
    expect((await worker.stop()).incomplete).toBe(true);
    expect(repair).toHaveBeenCalledOnce();
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("repairs pending audio before stopping", async () => {
    const { worker, repair, acknowledge, list } = setup();
    list.mockResolvedValueOnce(await list()).mockResolvedValue([]);
    worker.persistenceFailed();
    expect(await worker.stop()).toEqual({ incomplete: false });
    expect(repair).toHaveBeenCalledOnce();
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("releases retained batch-only chunks without transcribing during the meeting", async () => {
    const { worker, list, repair, acknowledge } = setup();
    list.mockResolvedValueOnce(await list()).mockResolvedValue([]);
    list.mockClear();
    worker.batchOnly(true);
    await worker.tick();
    expect(list).toHaveBeenCalledOnce();
    expect(repair).not.toHaveBeenCalled();
    expect(await worker.stop()).toEqual({ incomplete: false });
    expect(repair).not.toHaveBeenCalled();
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("processes zero-retention batch-only capture before native cleanup", async () => {
    const { worker, repair, acknowledge } = setup();
    worker.batchOnly(false);
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
    expect((await worker.stop()).incomplete).toBe(true);
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
    expect(await worker.stop()).toEqual({ incomplete: false });
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
  expect(await worker.stop()).toEqual({ incomplete: true });
  expect(list).toHaveBeenCalledOnce();
  expect(acknowledge).not.toHaveBeenCalled();
});
