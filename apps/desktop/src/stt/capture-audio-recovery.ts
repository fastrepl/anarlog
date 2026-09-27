import type { RecoveryAudioChunk } from "@anlg/plugin-transcription";

export type RecoveryInterval = { start: number; end: number };

// All times are relative to the frontend capture's start. Chunks retain their
// native capture timestamp so renderer reloads and recorder restarts stay aligned.
export function chunkInterval(chunk: RecoveryAudioChunk, startedAt: number) {
  const offset = chunk.capture_started_at - startedAt;
  return { start: offset + chunk.start_ms, end: offset + chunk.end_ms };
}

export function createCaptureAudioRecovery(options: {
  startedAt: number;
  list: () => Promise<RecoveryAudioChunk[]>;
  acknowledge: (chunk: RecoveryAudioChunk) => Promise<void>;
  flush: () => Promise<void>;
  repair: (
    chunk: RecoveryAudioChunk,
    intervals: RecoveryInterval[],
    signal: AbortSignal,
  ) => Promise<void>;
  now?: () => number;
}) {
  const now = options.now ?? Date.now;
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running: Promise<boolean> | undefined;
  let active = false;
  let online = true;
  let confirmedThrough = 0;
  let acknowledgedThrough = 0;
  let gapStart: number | undefined;
  let gaps: RecoveryInterval[] = [];
  let pending = false;
  let failed = false;
  let retryAt = 0;
  let recoverThrough = 0;
  let revision = 0;

  const elapsed = () => Math.max(0, now() - options.startedAt);
  const markGap = () => {
    revision += 1;
    gapStart ??= Math.max(acknowledgedThrough, confirmedThrough - 1_000);
    pending = true;
  };
  const closeGap = () => {
    if (gapStart === undefined) return;
    revision += 1;
    gaps.push({ start: gapStart, end: elapsed() });
    gapStart = undefined;
    // Adjacent incidents share one interval; outage count cannot grow RAM.
    if (gaps.length > 128)
      gaps = [{ start: gaps[0]!.start, end: gaps[gaps.length - 1]!.end }];
  };

  const process = async (settle: boolean) => {
    const chunks = await options.list();
    pending = gapStart !== undefined || gaps.length > 0;
    for (const chunk of chunks) {
      controller.signal.throwIfAborted();
      const range = chunkInterval(chunk, options.startedAt);
      if (!settle && range.end > elapsed() - 10_000) continue;
      await options.flush();
      controller.signal.throwIfAborted();
      const repairRevision = revision;
      const intervals = [
        ...gaps,
        ...(recoverThrough > range.start
          ? [{ start: range.start, end: recoverThrough }]
          : []),
        ...(gapStart === undefined
          ? []
          : [{ start: gapStart, end: elapsed() }]),
      ]
        .map((gap) => ({
          start: Math.max(gap.start, range.start),
          end: Math.min(gap.end, range.end),
        }))
        .filter((gap) => gap.start < gap.end);
      let coveredThrough = Math.max(range.start, confirmedThrough);
      for (const interval of [...intervals].sort((a, b) => a.start - b.start)) {
        if (interval.start <= coveredThrough)
          coveredThrough = Math.max(coveredThrough, interval.end);
      }
      if (!settle && coveredThrough < range.end) continue;
      if (intervals.length > 0) {
        pending = true;
        if (!online || now() < retryAt) return false;
        await options.repair(chunk, intervals, controller.signal);
        controller.signal.throwIfAborted();
      }
      if (repairRevision !== revision) return false;
      // Network success alone is insufficient: repair resolves after SQLite commits.
      await options.acknowledge(chunk);
      acknowledgedThrough = Math.max(acknowledgedThrough, range.end);
      gaps = gaps.flatMap((gap) =>
        gap.end <= range.end
          ? []
          : [{ start: Math.max(gap.start, range.end), end: gap.end }],
      );
      if (gapStart !== undefined) gapStart = Math.max(gapStart, range.end);
    }
    if (settle && chunks.length < 128) gaps = [];
    pending = gapStart !== undefined || gaps.length > 0 || chunks.length >= 128;
    return settle && chunks.length >= 128;
  };

  const tick = (settle = false) => {
    if (running) return running;
    running = process(settle)
      .catch((error) => {
        if (controller.signal.aborted) return false;
        pending = true;
        retryAt = now() + 30_000;
        console.warn("[listener] audio recovery deferred", error);
        return false;
      })
      .finally(() => {
        running = undefined;
        if (active) timer = setTimeout(() => void tick(), 5_000);
      });
    return running;
  };

  return {
    start() {
      if (active || controller.signal.aborted) return;
      active = true;
      timer = setTimeout(() => void tick(), 5_000);
    },
    persistedThrough(endMs: number) {
      confirmedThrough = Math.max(confirmedThrough, endMs);
    },
    interrupted() {
      online = false;
      markGap();
    },
    batchOnly() {
      online = true;
      markGap();
    },
    recoverPending() {
      recoverThrough = elapsed();
      pending = true;
      online = true;
    },
    connected() {
      closeGap();
      online = true;
      retryAt = 0;
    },
    persistenceFailed() {
      markGap();
      closeGap();
    },
    storageFailed() {
      failed = true;
      markGap();
    },
    async stop(retainAudio: boolean) {
      active = false;
      clearTimeout(timer);
      if (!retainAudio) {
        controller.abort();
        await running;
        return {
          incomplete:
            pending || failed || gapStart !== undefined || gaps.length > 0,
        };
      }
      closeGap();
      await running;
      while (await tick(true)) {
        controller.signal.throwIfAborted();
      }
      return { incomplete: pending || failed };
    },
    cancel() {
      active = false;
      clearTimeout(timer);
      controller.abort();
    },
    tick,
  };
}
