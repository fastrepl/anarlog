import { describe, expect, it, vi } from "vitest";

import type { LiveGaps } from "@anlg/plugin-transcription";

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
  let gaps: LiveGaps = {
    capture_started_at: 0,
    closed: [],
    open_since_ms: null,
  };
  const list = vi.fn(async () => [chunk]);
  const liveGaps = vi.fn(async () => gaps);
  const acknowledge = vi.fn(async (_chunk: { id: string }) => {});
  const flush = vi.fn(async () => {});
  const repair = vi.fn(async (_chunk, _gaps, _signal: AbortSignal) => {});
  const worker = createCaptureAudioRecovery({
    startedAt: 0,
    list,
    liveGaps,
    acknowledge,
    flush,
    repair,
    onStatus: vi.fn(),
    now: () => now,
  });
  return {
    worker,
    list,
    liveGaps,
    acknowledge,
    flush,
    repair,
    setNow: (value: number) => {
      now = value;
    },
    // Mirrors what listener-core persists: the listener dropped at `start`
    // (already rewound 1s before the last confirmed word) and, once `end` is
    // known, reattached at `end`.
    setLiveGaps: (value: Partial<LiveGaps>) => {
      gaps = { ...gaps, ...value };
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

  it("repairs only the interval Rust reports as missing, even mid-outage", async () => {
    const { worker, repair, acknowledge, setNow, setLiveGaps } = setup();
    worker.persistedThrough(20_000);
    setLiveGaps({ open_since_ms: 19_000 });
    setNow(90_000);
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 19_000, end: 60_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("aligns Rust gap timestamps when the renderer started earlier than the capture", async () => {
    const { worker, repair, setLiveGaps } = setup();
    setLiveGaps({
      capture_started_at: 5_000,
      closed: [{ start_ms: 10_000, end_ms: 20_000 }],
    });
    worker.persistedThrough(70_000);
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 15_000, end: 25_000 }]);
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

  it("keeps a healthy-looking chunk when an outage starts while it is being flushed", async () => {
    const { worker, flush, repair, acknowledge, setLiveGaps } = setup();
    worker.persistedThrough(70_000);
    flush.mockImplementationOnce(async () => {
      setLiveGaps({ open_since_ms: 59_000 });
    });
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 59_000, end: 60_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("reports incomplete when an outage reaches a chunk during the final pass", async () => {
    const { worker, flush, acknowledge, setLiveGaps } = setup();
    worker.persistedThrough(70_000);
    flush.mockImplementationOnce(async () => {
      setLiveGaps({ open_since_ms: 59_000 });
    });
    expect(await worker.stop(true)).toEqual({ incomplete: true });
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("stays incomplete after a failed transcript write until its audio is finalized", async () => {
    const { worker, list, acknowledge } = setup();
    worker.persistedThrough(30_000);
    worker.persistenceFailed();
    list.mockResolvedValueOnce([]);
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    expect((await worker.stop(false)).incomplete).toBe(true);
  });

  it("processes batch-only capture in bounded chunks during the meeting", async () => {
    const { worker, repair, acknowledge, setLiveGaps } = setup();
    setLiveGaps({ open_since_ms: 0 });
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 0, end: 60_000 }]);
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

  it("does not acknowledge audio when another outage arrives during repair", async () => {
    const { worker, repair, acknowledge, setLiveGaps } = setup();
    worker.recoverPending();
    repair.mockImplementation(async () =>
      setLiveGaps({ open_since_ms: 50_000 }),
    );
    await worker.tick();
    expect(acknowledge).not.toHaveBeenCalled();
    expect((await worker.stop(false)).incomplete).toBe(true);
  });

  it("waits for a fresh outage to clear before touching recent audio", async () => {
    const { worker, repair, acknowledge, setLiveGaps, setNow } = setup();
    worker.persistedThrough(70_000);
    setLiveGaps({ open_since_ms: 55_000 });
    setNow(62_000);
    await worker.tick();
    expect(repair).not.toHaveBeenCalled();
    expect(acknowledge).not.toHaveBeenCalled();
    setNow(90_000);
    await worker.tick();
    expect(repair.mock.calls[0]?.[1]).toEqual([{ start: 55_000, end: 60_000 }]);
    expect(acknowledge).toHaveBeenCalledOnce();
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

it("drains a one-hour outage recorded by Rust when stopping", async () => {
  const { worker, list, acknowledge, repair, setNow, setLiveGaps } = setup();
  let chunks = Array.from({ length: 60 }, (_, index) => ({
    id: `${index}.mp3`,
    path: `/chunk-${index}.mp3`,
    capture_started_at: 0,
    start_ms: index * 60_000,
    audio_start_ms: index * 60_000,
    end_ms: (index + 1) * 60_000,
  }));
  list.mockImplementation(async () => chunks);
  acknowledge.mockImplementation(async (chunk) => {
    chunks = chunks.filter((candidate) => candidate.id !== chunk.id);
  });
  setLiveGaps({
    closed: [{ start_ms: 90_000, end_ms: 200_000 }],
    open_since_ms: 3_500_000,
  });
  setNow(3_600_000);
  expect(await worker.stop(true)).toEqual({ incomplete: false });
  expect(repair.mock.calls.map(([chunk]) => chunk.id)).toEqual([
    "1.mp3",
    "2.mp3",
    "3.mp3",
    "58.mp3",
    "59.mp3",
  ]);
  expect(chunks).toEqual([]);
});

it("stops draining when repair keeps failing instead of spinning on the same page", async () => {
  const { worker, list, acknowledge, repair, setLiveGaps } = setup();
  const chunk = (await list())[0]!;
  list.mockResolvedValue(Array.from({ length: 128 }, () => chunk));
  list.mockClear();
  repair.mockRejectedValue(new Error("offline"));
  setLiveGaps({ open_since_ms: 0 });
  expect(await worker.stop(true)).toEqual({ incomplete: true });
  expect(list).toHaveBeenCalledOnce();
  expect(acknowledge).not.toHaveBeenCalled();
});
