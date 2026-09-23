import type { LiveGaps, RecoveryAudioChunk } from "@anlg/plugin-transcription";

export type RecoveryInterval = { start: number; end: number };

// All times are relative to the frontend capture's start. Chunks retain their
// native capture timestamp so renderer reloads and recorder restarts stay aligned.
export function chunkInterval(chunk: RecoveryAudioChunk, startedAt: number) {
  const offset = chunk.capture_started_at - startedAt;
  return { start: offset + chunk.start_ms, end: offset + chunk.end_ms };
}

const MAX_LOCAL_GAPS = 128;
const PAGE_SIZE = 128;

function clip(gaps: RecoveryInterval[], range: RecoveryInterval) {
  return gaps
    .map((gap) => ({
      start: Math.max(gap.start, range.start),
      end: Math.min(gap.end, range.end),
    }))
    .filter((gap) => gap.start < gap.end)
    .sort((a, b) => a.start - b.start);
}

function sameIntervals(a: RecoveryInterval[], b: RecoveryInterval[]) {
  return (
    a.length === b.length &&
    a.every((gap, i) => gap.start === b[i]!.start && gap.end === b[i]!.end)
  );
}

// Rust owns "when was there no live listener" (see LiveGaps in listener-core).
// The renderer only adds incidents Rust cannot see: transcript writes that
// failed, and the backlog before it reattached to an already-running capture.
export function createCaptureAudioRecovery(options: {
  startedAt: number;
  list: () => Promise<RecoveryAudioChunk[]>;
  liveGaps: () => Promise<LiveGaps>;
  acknowledge: (chunk: RecoveryAudioChunk) => Promise<void>;
  flush: () => Promise<void>;
  repair: (
    chunk: RecoveryAudioChunk,
    intervals: RecoveryInterval[],
    signal: AbortSignal,
  ) => Promise<void>;
  onStatus: (status: "waiting" | "repairing" | "complete") => void;
  now?: () => number;
}) {
  const now = options.now ?? Date.now;
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running: Promise<boolean> | undefined;
  let active = false;
  let confirmedThrough = 0;
  let localGaps: RecoveryInterval[] = [];
  let retryAt = 0;
  let unresolved = false;
  let checked = false;
  let storageFailed = false;

  const elapsed = () => Math.max(0, now() - options.startedAt);
  const addLocalGap = (start: number, end: number) => {
    if (start < end) localGaps.push({ start, end });
    if (localGaps.length > MAX_LOCAL_GAPS)
      localGaps = [
        {
          start: localGaps[0]!.start,
          end: localGaps[localGaps.length - 1]!.end,
        },
      ];
    unresolved = true;
    options.onStatus("waiting");
  };
  const gapsFor = (live: LiveGaps, range: RecoveryInterval) => {
    const offset = live.capture_started_at - options.startedAt;
    return clip(
      [
        ...live.closed.map((gap) => ({
          start: offset + gap.start_ms,
          end: offset + gap.end_ms,
        })),
        ...(live.open_since_ms === null
          ? []
          : [{ start: offset + live.open_since_ms, end: elapsed() }]),
        ...localGaps,
      ],
      range,
    );
  };
  // A chunk is releasable once every part of it is either durably transcribed
  // live or scheduled for repair; otherwise a late-arriving gap could still
  // reach into it.
  const covered = (range: RecoveryInterval, intervals: RecoveryInterval[]) => {
    let through = Math.max(range.start, confirmedThrough);
    for (const gap of intervals)
      if (gap.start <= through) through = Math.max(through, gap.end);
    return through >= range.end;
  };

  const process = async (settle: boolean) => {
    const chunks = await options.list();
    let live = await options.liveGaps();
    // Chunks with a gap that could not be repaired or released this pass.
    let remaining = 0;
    let released = 0;
    for (const chunk of chunks) {
      controller.signal.throwIfAborted();
      const range = chunkInterval(chunk, options.startedAt);
      // Recent audio may still be transcribed live; leave it alone for now.
      if (!settle && range.end > elapsed() - 10_000) {
        if (gapsFor(live, range).length > 0) remaining += 1;
        continue;
      }
      await options.flush();
      controller.signal.throwIfAborted();
      const intervals = gapsFor(live, range);
      if (!settle && !covered(range, intervals)) {
        if (intervals.length > 0) remaining += 1;
        continue;
      }
      if (intervals.length > 0) {
        if (now() < retryAt) {
          unresolved = true;
          return false;
        }
        unresolved = true;
        options.onStatus("repairing");
        await options.repair(chunk, intervals, controller.signal);
        controller.signal.throwIfAborted();
      }
      // Acknowledging deletes the audio, so make sure no outage reached into this
      // chunk since the gaps were read.
      live = await options.liveGaps();
      if (!sameIntervals(intervals, gapsFor(live, range))) return false;
      // Network success alone is insufficient: repair resolves after SQLite commits.
      await options.acknowledge(chunk);
      released += 1;
      localGaps = clip(localGaps, { start: range.end, end: Infinity });
    }
    // Once capture has ended, no chunk will ever cover audio past the last one.
    if (settle && chunks.length < PAGE_SIZE) localGaps = [];
    unresolved =
      remaining > 0 || chunks.length >= PAGE_SIZE || localGaps.length > 0;
    checked = true;
    if (unresolved) options.onStatus("waiting");
    else if (!storageFailed) options.onStatus("complete");
    return settle && released > 0 && chunks.length >= PAGE_SIZE;
  };

  const tick = (settle = false) => {
    if (running) return running;
    running = process(settle)
      .catch((error) => {
        if (controller.signal.aborted) return false;
        unresolved = true;
        retryAt = now() + 30_000;
        options.onStatus("waiting");
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
    recoverPending() {
      addLocalGap(0, elapsed());
    },
    persistenceFailed() {
      addLocalGap(confirmedThrough - 1_000, elapsed());
    },
    storageFailed() {
      storageFailed = true;
      unresolved = true;
      options.onStatus("waiting");
    },
    async stop(retainAudio: boolean) {
      active = false;
      clearTimeout(timer);
      if (!retainAudio) {
        controller.abort();
        await running;
        // Nothing was released yet, so ask Rust directly whether live missed anything.
        if (!checked && !unresolved)
          unresolved = await options.liveGaps().then(
            (live) => live.closed.length > 0 || live.open_since_ms !== null,
            () => true,
          );
        return { incomplete: unresolved || storageFailed };
      }
      await running;
      while (await tick(true)) {
        controller.signal.throwIfAborted();
      }
      return { incomplete: unresolved || storageFailed };
    },
    cancel() {
      active = false;
      clearTimeout(timer);
      controller.abort();
    },
    tick,
  };
}
