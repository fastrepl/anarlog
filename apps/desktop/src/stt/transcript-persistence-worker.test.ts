import { afterEach, describe, expect, it, vi } from "vitest";

import type { LiveTranscriptDelta } from "@anlg/plugin-transcription";

import { createTranscriptPersistenceWorker } from "./transcript-persistence-worker";
import type { SpeakerHintWithId, WordWithId } from "./types";
import { createTranscriptAccumulator } from "./utils";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function delta(
  id: string,
  options?: {
    channel?: number;
    replacedIds?: string[];
    speakerIndex?: number;
    startMs?: number;
    text?: string;
    partialText?: string;
  },
): LiveTranscriptDelta {
  const startMs = options?.startMs ?? (Number(id.replace(/\D/g, "")) || 0);
  return {
    new_words: [
      {
        id,
        text: options?.text ?? id,
        start_ms: startMs,
        end_ms: startMs + 1,
        channel: options?.channel ?? 0,
        state: "final",
        speaker_index: options?.speakerIndex,
      },
    ],
    replaced_ids: options?.replacedIds ?? [],
    partials: options?.partialText
      ? [
          {
            text: options.partialText,
            start_ms: 0,
            end_ms: 1,
            channel: 0,
          },
        ]
      : [],
  };
}

function createTranscriptStore(
  initialWords: WordWithId[],
  initialHints: SpeakerHintWithId[],
) {
  const transcript = {
    words: JSON.stringify(initialWords),
    speaker_hints: JSON.stringify(initialHints),
  };

  return {
    getCell: (
      _tableId: "transcripts",
      _rowId: string,
      cellId: "words" | "speaker_hints",
    ) => transcript[cellId],
    setCell: (
      _tableId: "transcripts",
      _rowId: string,
      cellId: "words" | "speaker_hints",
      value: string,
    ) => {
      transcript[cellId] = value;
    },
    readWords: () => JSON.parse(transcript.words) as WordWithId[],
    readHints: () =>
      JSON.parse(transcript.speaker_hints) as SpeakerHintWithId[],
  };
}

function createImmediateTranscriptPersistenceWorker(
  persist: Parameters<typeof createTranscriptPersistenceWorker>[0],
  onError: Parameters<typeof createTranscriptPersistenceWorker>[1],
  options: Parameters<typeof createTranscriptPersistenceWorker>[2] = {},
) {
  return createTranscriptPersistenceWorker(persist, onError, {
    batchWindowMs: 0,
    ...options,
  });
}

describe("createTranscriptPersistenceWorker", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("persists one coalesced journal chunk per batch window", async () => {
    vi.useFakeTimers();
    const persist = vi.fn(async (_delta: LiveTranscriptDelta) => undefined);
    const afterFlush = vi.fn(async () => undefined);
    const worker = createTranscriptPersistenceWorker(persist, vi.fn(), {
      afterFlush,
      batchWindowMs: 50,
    });

    worker.enqueue(delta("word-1"));
    worker.enqueue(delta("word-2"));
    await vi.advanceTimersByTimeAsync(49);
    expect(persist).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await worker.flush();

    expect(persist).toHaveBeenCalledOnce();
    expect(persist.mock.calls[0]?.[0].new_words.map((word) => word.id)).toEqual(
      ["word-1", "word-2"],
    );
    expect(afterFlush).toHaveBeenCalledOnce();
  });

  it("does not persist UI-only partials", async () => {
    const persist = vi.fn(async (_delta: LiveTranscriptDelta) => undefined);
    const worker = createImmediateTranscriptPersistenceWorker(persist, vi.fn());

    worker.enqueue({
      new_words: [],
      replaced_ids: [],
      partials: [
        {
          text: "still speaking",
          start_ms: 0,
          end_ms: 1,
          channel: 0,
        },
      ],
    });
    await worker.flush();

    expect(persist).not.toHaveBeenCalled();
  });

  it("coalesces a long burst behind one in-flight write and flushes later work", async () => {
    const firstWrite = deferred();
    const secondWrite = deferred();
    const writes: LiveTranscriptDelta[] = [];
    let activeWrites = 0;
    let maximumActiveWrites = 0;
    const persist = vi.fn(async (nextDelta: LiveTranscriptDelta) => {
      writes.push(nextDelta);
      activeWrites += 1;
      maximumActiveWrites = Math.max(maximumActiveWrites, activeWrites);
      if (writes.length === 1) {
        await firstWrite.promise;
      } else if (writes.length === 2) {
        await secondWrite.promise;
      }
      activeWrites -= 1;
    });
    const worker = createImmediateTranscriptPersistenceWorker(persist, vi.fn());

    worker.enqueue(delta("word-0"));
    await vi.waitFor(() => expect(persist).toHaveBeenCalledTimes(1));

    for (let index = 1; index <= 1_000; index += 1) {
      worker.enqueue(
        delta(`word-${index}`, {
          partialText: `partial-${index}`,
        }),
      );
    }

    const flushed = worker.flush();
    firstWrite.resolve();
    await vi.waitFor(() => expect(persist).toHaveBeenCalledTimes(2));

    expect(writes[1]?.new_words).toHaveLength(1_000);
    expect(writes[1]?.partials).toEqual([]);

    secondWrite.resolve();
    worker.enqueue(delta("word-1001"));
    await flushed;

    expect(persist).toHaveBeenCalledTimes(3);
    expect(writes[2]?.new_words.map((word) => word.id)).toEqual(["word-1001"]);
    expect(maximumActiveWrites).toBe(1);
  });

  it("collapses corrections and append runs into their canonical final delta", async () => {
    const firstWrite = deferred();
    const writes: LiveTranscriptDelta[] = [];
    const worker = createImmediateTranscriptPersistenceWorker(
      async (nextDelta) => {
        writes.push(nextDelta);
        if (writes.length === 1) {
          await firstWrite.promise;
        }
      },
      vi.fn(),
    );

    worker.enqueue(delta("blocker"));
    worker.enqueue(delta("word-1", { text: "latest" }));
    worker.enqueue(delta("word-2"));
    worker.enqueue(
      delta("word-3", {
        replacedIds: ["word-1", "word-2"],
        partialText: "correction partial",
      }),
    );
    worker.enqueue(delta("word-4", { text: "old" }));
    worker.enqueue(delta("word-4", { text: "new" }));
    worker.enqueue(delta("word-5", { partialText: "latest partial" }));
    firstWrite.resolve();
    await worker.flush();

    expect(writes).toHaveLength(2);
    expect(writes[1]?.new_words).toEqual([
      expect.objectContaining({ id: "word-3" }),
      expect.objectContaining({ id: "word-4", text: "new" }),
      expect.objectContaining({ id: "word-5" }),
    ]);
    expect(writes[1]?.replaced_ids).toEqual(["word-1", "word-2"]);
    expect(writes[1]?.partials).toEqual([]);
  });

  it("bounds a delayed correction burst while preserving final words and speaker hints", async () => {
    const firstWrite = deferred();
    const writes: LiveTranscriptDelta[] = [];
    const store = createTranscriptStore(
      [
        {
          id: "word-0",
          text: "initial",
          start_ms: 0,
          end_ms: 1,
          channel: 0,
        },
      ],
      [
        {
          id: "word-0:provider_speaker_index",
          word_id: "word-0",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 0, speaker_index: 2 }),
        },
        {
          id: "word-0:user_speaker_assignment:segment",
          word_id: "word-0",
          type: "user_speaker_assignment",
          value: JSON.stringify({
            human_id: "human-1",
            scope: "segment",
            word_ids: ["word-0"],
          }),
        },
      ],
    );
    let activeWrites = 0;
    let maximumActiveWrites = 0;
    const persist = vi.fn(async (nextDelta: LiveTranscriptDelta) => {
      writes.push(nextDelta);
      activeWrites += 1;
      maximumActiveWrites = Math.max(maximumActiveWrites, activeWrites);
      if (writes.length === 1) {
        await firstWrite.promise;
      }

      const accumulator = createTranscriptAccumulator(store, "transcript-1");
      accumulator.applyLiveDelta(nextDelta);
      accumulator.dispose();
      activeWrites -= 1;
    });
    const worker = createImmediateTranscriptPersistenceWorker(persist, vi.fn());

    worker.enqueue(
      delta("blocker", {
        channel: 1,
        speakerIndex: 0,
        startMs: 10_000,
      }),
    );
    await vi.waitFor(() => expect(persist).toHaveBeenCalledTimes(1));

    for (let index = 1; index <= 1_000; index += 1) {
      worker.enqueue(
        delta(`word-${index}`, {
          replacedIds: [`word-${index - 1}`],
          speakerIndex: 2,
          text: `correction-${index}`,
        }),
      );
    }

    expect(persist).toHaveBeenCalledTimes(1);
    firstWrite.resolve();
    await worker.flush();

    expect(persist).toHaveBeenCalledTimes(2);
    expect(writes[1]?.replaced_ids).toEqual(["word-0"]);
    expect(writes[1]?.new_words).toEqual([
      expect.objectContaining({
        id: "word-1000",
        text: "correction-1000",
        speaker_index: 2,
      }),
    ]);
    expect(store.readWords()).toEqual([
      expect.objectContaining({
        id: "word-1000",
        text: "correction-1000",
      }),
      expect.objectContaining({ id: "blocker" }),
    ]);

    const hints = store.readHints();
    expect(hints).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          word_id: "word-1000",
          type: "provider_speaker_index",
        }),
        expect.objectContaining({
          word_id: "word-1000",
          type: "user_speaker_assignment",
        }),
      ]),
    );
    const assignment = hints.find(
      (hint) => hint.type === "user_speaker_assignment",
    );
    expect(JSON.parse(String(assignment?.value))).toEqual({
      human_id: "human-1",
      scope: "segment",
      word_ids: ["word-1000"],
    });
    expect(maximumActiveWrites).toBe(1);
  });

  it("retains a persisted root while collapsing its pending replacement lineage", async () => {
    const firstWrite = deferred();
    const writes: LiveTranscriptDelta[] = [];
    const store = createTranscriptStore(
      [
        {
          id: "word-x",
          text: "persisted",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
        },
      ],
      [
        {
          id: "word-x:provider_speaker_index",
          word_id: "word-x",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 0, speaker_index: 2 }),
        },
        {
          id: "word-x:user_speaker_assignment:segment",
          word_id: "word-x",
          type: "user_speaker_assignment",
          value: JSON.stringify({
            human_id: "human-1",
            scope: "segment",
            word_ids: ["word-x"],
          }),
        },
      ],
    );
    const persist = vi.fn(async (nextDelta: LiveTranscriptDelta) => {
      writes.push(nextDelta);
      if (writes.length === 1) {
        await firstWrite.promise;
      }

      const accumulator = createTranscriptAccumulator(store, "transcript-1");
      accumulator.applyLiveDelta(nextDelta);
      accumulator.dispose();
    });
    const worker = createImmediateTranscriptPersistenceWorker(persist, vi.fn());

    worker.enqueue(delta("blocker", { channel: 1, startMs: 10_000 }));
    await vi.waitFor(() => expect(persist).toHaveBeenCalledTimes(1));
    worker.enqueue(
      delta("word-x", {
        speakerIndex: 2,
        startMs: 0,
        text: "pending update",
      }),
    );
    worker.enqueue(
      delta("word-y", {
        replacedIds: ["word-x"],
        speakerIndex: 2,
        startMs: 0,
        text: "intermediate correction",
      }),
    );
    worker.enqueue(
      delta("word-z", {
        replacedIds: ["word-y"],
        speakerIndex: 2,
        startMs: 0,
        text: "final correction",
      }),
    );

    firstWrite.resolve();
    await worker.flush();

    expect(writes).toHaveLength(2);
    expect(writes[1]).toEqual(
      expect.objectContaining({
        new_words: [
          expect.objectContaining({
            id: "word-z",
            text: "final correction",
          }),
        ],
        replaced_ids: ["word-x"],
      }),
    );
    expect(store.readWords()).toEqual([
      expect.objectContaining({ id: "word-z", text: "final correction" }),
      expect.objectContaining({ id: "blocker" }),
    ]);

    const hints = store.readHints();
    expect(
      hints.some(
        (hint) => hint.word_id === "word-x" || hint.word_id === "word-y",
      ),
    ).toBe(false);
    expect(hints).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          word_id: "word-z",
          type: "provider_speaker_index",
        }),
        expect.objectContaining({
          word_id: "word-z",
          type: "user_speaker_assignment",
        }),
      ]),
    );
  });

  it("fails closed when a stalled persistence backlog exceeds its cap", async () => {
    const firstWrite = deferred();
    const onError = vi.fn();
    const persist = vi.fn(async () => {
      await firstWrite.promise;
    });
    const worker = createImmediateTranscriptPersistenceWorker(persist, onError);

    worker.enqueue(delta("blocker"));
    await vi.waitFor(() => expect(persist).toHaveBeenCalledTimes(1));

    for (let index = 0; index <= 10_000; index += 1) {
      worker.enqueue(delta(`backlog-${index}`));
    }
    worker.enqueue(delta("ignored-after-overflow"));

    expect(onError).toHaveBeenCalledOnce();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({
        message:
          "Transcript persistence backlog exceeded its safe memory bounds",
      }),
    );
    firstWrite.resolve();
    await worker.flush();
    expect(persist).toHaveBeenCalledOnce();
  });

  it("reports a failed write and continues draining", async () => {
    const error = new Error("write failed");
    const onError = vi.fn();
    const persist = vi
      .fn<(delta: LiveTranscriptDelta) => Promise<void>>()
      .mockRejectedValueOnce(error)
      .mockResolvedValue(undefined);
    const worker = createImmediateTranscriptPersistenceWorker(persist, onError);

    worker.enqueue(delta("word-1", { replacedIds: ["old-word"] }));
    worker.enqueue(delta("word-2"));
    await worker.flush();

    expect(onError).toHaveBeenCalledWith(error);
    expect(persist).toHaveBeenCalledTimes(2);
  });

  it("times out a hung persist without wedging flush", async () => {
    vi.useFakeTimers();
    let rejectPersist!: (error: unknown) => void;
    const persist = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectPersist = reject;
        }),
    );
    const onError = vi.fn();
    const worker = createImmediateTranscriptPersistenceWorker(
      persist,
      onError,
      {
        persistTimeoutMs: 50,
        flushTimeoutMs: 100,
      },
    );

    worker.enqueue(delta("word-1"));
    const flushed = worker.flush();
    await vi.advanceTimersByTimeAsync(49);
    expect(onError).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await expect(flushed).resolves.toBeUndefined();
    expect(onError).toHaveBeenCalledOnce();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({
        message: "Transcript persistence timed out after 50ms",
      }),
    );

    worker.enqueue(delta("ignored-after-timeout"));
    expect(persist).toHaveBeenCalledOnce();

    rejectPersist(new Error("late persist rejection"));
    await Promise.resolve();
    expect(onError).toHaveBeenCalledOnce();
  });

  it("bounds flush independently of the persist deadline", async () => {
    vi.useFakeTimers();
    let rejectPersist!: (error: unknown) => void;
    const persist = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectPersist = reject;
        }),
    );
    const onError = vi.fn();
    const worker = createImmediateTranscriptPersistenceWorker(
      persist,
      onError,
      {
        persistTimeoutMs: 1_000,
        flushTimeoutMs: 25,
      },
    );

    worker.enqueue(delta("word-1"));
    const flushed = worker.flush();
    await vi.advanceTimersByTimeAsync(25);

    await expect(flushed).resolves.toBeUndefined();
    expect(onError).toHaveBeenCalledOnce();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({
        message: "Transcript persistence flush timed out after 25ms",
      }),
    );

    rejectPersist(new Error("late persist rejection"));
    await Promise.resolve();
    expect(onError).toHaveBeenCalledOnce();
  });
});
