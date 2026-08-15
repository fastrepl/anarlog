import type { LiveTranscriptDelta } from "@anlg/plugin-transcription";

type PendingTranscriptWrite = {
  wordsById: Map<
    string,
    {
      words: LiveTranscriptDelta["new_words"];
      mayExistInPersistedBase: boolean;
    }
  >;
  replacedRootIds: Set<string>;
  partials: LiveTranscriptDelta["partials"];
  wordCount: number;
  textLength: number;
};

const MAX_PENDING_WORDS = 10_000;
const MAX_PENDING_TEXT_LENGTH = 1_000_000;
const MAX_PENDING_REPLACED_IDS = 10_000;
const TRANSCRIPT_BATCH_WINDOW_MS = 250;
const TRANSCRIPT_PERSIST_TIMEOUT_MS = 15_000;
const TRANSCRIPT_FLUSH_TIMEOUT_MS = 20_000;

class TranscriptPersistenceTimeoutError extends Error {}

export function createTranscriptPersistenceWorker(
  persist: (delta: LiveTranscriptDelta) => Promise<void>,
  onError: (error: unknown) => void,
  options: {
    afterFlush?: () => Promise<void>;
    batchWindowMs?: number;
    persistTimeoutMs?: number;
    flushTimeoutMs?: number;
  } = {},
) {
  const batchWindowMs = options.batchWindowMs ?? TRANSCRIPT_BATCH_WINDOW_MS;
  const persistTimeoutMs =
    options.persistTimeoutMs ?? TRANSCRIPT_PERSIST_TIMEOUT_MS;
  const flushTimeoutMs = options.flushTimeoutMs ?? TRANSCRIPT_FLUSH_TIMEOUT_MS;
  let pendingWrite: PendingTranscriptWrite | null = null;
  let drainPromise: Promise<void> | null = null;
  let batchTimer: ReturnType<typeof setTimeout> | null = null;
  let overflowed = false;
  let timedOut = false;
  let cancelActivePersist: ((error: Error) => void) | null = null;

  const reportError = (error: unknown) => {
    try {
      onError(error);
    } catch (callbackError) {
      console.error(
        "[listener] transcript persistence error handler failed",
        callbackError,
      );
    }
  };
  const stopAfterTimeout = (error: TranscriptPersistenceTimeoutError) => {
    if (timedOut) {
      return;
    }

    timedOut = true;
    pendingWrite = null;
    if (batchTimer) {
      clearTimeout(batchTimer);
      batchTimer = null;
    }
    reportError(error);
    cancelActivePersist?.(error);
  };
  const persistWithinDeadline = (delta: LiveTranscriptDelta) =>
    new Promise<void>((resolve, reject) => {
      let settled = false;
      let timeoutId: ReturnType<typeof setTimeout>;
      const finish = (complete: (value?: never) => void, value?: unknown) => {
        if (settled) {
          return;
        }
        settled = true;
        clearTimeout(timeoutId);
        if (cancelActivePersist === cancel) {
          cancelActivePersist = null;
        }
        complete(value as never);
      };
      const cancel = (error: Error) => finish(reject, error);
      cancelActivePersist = cancel;
      timeoutId = setTimeout(
        () =>
          cancel(
            new TranscriptPersistenceTimeoutError(
              `Transcript persistence timed out after ${persistTimeoutMs}ms`,
            ),
          ),
        persistTimeoutMs,
      );

      void Promise.resolve()
        .then(() => persist(delta))
        .then(
          () => finish(resolve),
          (error) => finish(reject, error),
        );
    });

  const drain = async () => {
    while (pendingWrite && !timedOut) {
      const write = pendingWrite;
      pendingWrite = null;

      try {
        await persistWithinDeadline(toDelta(write));
      } catch (error) {
        if (error instanceof TranscriptPersistenceTimeoutError) {
          stopAfterTimeout(error);
        } else {
          reportError(error);
        }
      }
    }
  };

  const startDrain = (immediate = false) => {
    if (drainPromise || timedOut) {
      return;
    }
    if (!immediate && batchWindowMs > 0) {
      if (batchTimer) {
        return;
      }
      batchTimer = setTimeout(() => {
        batchTimer = null;
        startDrain(true);
      }, batchWindowMs);
      return;
    }

    let resolveDrain!: () => void;
    let rejectDrain!: (error: unknown) => void;
    const currentDrain = new Promise<void>((resolve, reject) => {
      resolveDrain = resolve;
      rejectDrain = reject;
    });
    drainPromise = currentDrain;
    const finishDrain = () => {
      if (drainPromise !== currentDrain) {
        return;
      }

      drainPromise = null;
      if (pendingWrite) {
        startDrain();
      }
    };
    void currentDrain.then(finishDrain, finishDrain);
    void drain().then(resolveDrain, rejectDrain);
  };

  const enqueue = (delta: LiveTranscriptDelta) => {
    if (overflowed || timedOut) {
      return;
    }

    pendingWrite ??= createPendingWrite();
    mergeDelta(pendingWrite, delta);
    if (exceedsSafeBounds(pendingWrite)) {
      pendingWrite = null;
      overflowed = true;
      reportError(
        new Error(
          "Transcript persistence backlog exceeded its safe memory bounds",
        ),
      );
      return;
    }
    startDrain();
  };

  const flush = async () => {
    let timeoutId: ReturnType<typeof setTimeout>;
    const flushTimedOut = Symbol();
    const timeout = new Promise<typeof flushTimedOut>((resolve) => {
      timeoutId = setTimeout(() => resolve(flushTimedOut), flushTimeoutMs);
    });

    try {
      while (drainPromise || pendingWrite || batchTimer) {
        if (batchTimer) {
          clearTimeout(batchTimer);
          batchTimer = null;
        }
        if (pendingWrite && !drainPromise) {
          startDrain(true);
        }
        if (!drainPromise) {
          continue;
        }
        const result = await Promise.race([
          drainPromise.then(() => null),
          timeout,
        ]);
        if (result === flushTimedOut) {
          stopAfterTimeout(
            new TranscriptPersistenceTimeoutError(
              `Transcript persistence flush timed out after ${flushTimeoutMs}ms`,
            ),
          );
          return;
        }
      }

      if (!timedOut && options.afterFlush) {
        const result = await Promise.race([
          Promise.resolve()
            .then(options.afterFlush)
            .then(
              () => null,
              (error) => {
                reportError(error);
                return null;
              },
            ),
          timeout,
        ]);
        if (result === flushTimedOut) {
          stopAfterTimeout(
            new TranscriptPersistenceTimeoutError(
              `Transcript persistence flush timed out after ${flushTimeoutMs}ms`,
            ),
          );
        }
      }
    } finally {
      clearTimeout(timeoutId!);
    }
  };

  return { enqueue, flush };
}

function createPendingWrite(): PendingTranscriptWrite {
  return {
    wordsById: new Map(),
    replacedRootIds: new Set(),
    partials: [],
    wordCount: 0,
    textLength: 0,
  };
}

function mergeDelta(
  pendingWrite: PendingTranscriptWrite,
  delta: LiveTranscriptDelta,
) {
  for (const replacedId of delta.replaced_ids) {
    const pendingWords = pendingWrite.wordsById.get(replacedId);
    if (pendingWords) {
      removePendingWords(pendingWrite, replacedId, pendingWords.words);
      if (pendingWords.mayExistInPersistedBase) {
        pendingWrite.replacedRootIds.add(replacedId);
      }
    } else {
      pendingWrite.replacedRootIds.add(replacedId);
    }
  }

  const nextWordsById = new Map<string, LiveTranscriptDelta["new_words"]>();
  for (const word of delta.new_words) {
    const words = nextWordsById.get(word.id) ?? [];
    words.push(word);
    nextWordsById.set(word.id, words);
  }
  for (const [wordId, words] of nextWordsById) {
    const existing = pendingWrite.wordsById.get(wordId);
    if (existing) {
      removePendingWords(pendingWrite, wordId, existing.words);
    }

    pendingWrite.wordsById.set(wordId, {
      words,
      // A standalone upsert may target an ID already in SQLite. Replacement
      // outputs are pending-only descendants whose persisted roots were kept
      // when their inputs were removed above.
      mayExistInPersistedBase:
        existing?.mayExistInPersistedBase ?? delta.replaced_ids.length === 0,
    });
    pendingWrite.wordCount += words.length;
    pendingWrite.textLength += words.reduce(
      (length, word) => length + word.text.length,
      0,
    );
  }

  pendingWrite.textLength -= pendingWrite.partials.reduce(
    (length, word) => length + word.text.length,
    0,
  );
  pendingWrite.partials = [...delta.partials];
  pendingWrite.textLength += pendingWrite.partials.reduce(
    (length, word) => length + word.text.length,
    0,
  );
}

function toDelta(pendingWrite: PendingTranscriptWrite): LiveTranscriptDelta {
  const newWords: LiveTranscriptDelta["new_words"] = [];
  for (const { words } of pendingWrite.wordsById.values()) {
    newWords.push(...words);
  }

  return {
    new_words: newWords,
    replaced_ids: [...pendingWrite.replacedRootIds],
    partials: pendingWrite.partials,
  };
}

export function coalesceLiveTranscriptDeltas(
  deltas: readonly LiveTranscriptDelta[],
): LiveTranscriptDelta {
  const pendingWrite = createPendingWrite();
  for (const delta of deltas) mergeDelta(pendingWrite, delta);
  return toDelta(pendingWrite);
}

function removePendingWords(
  pendingWrite: PendingTranscriptWrite,
  wordId: string,
  words: LiveTranscriptDelta["new_words"],
) {
  pendingWrite.wordsById.delete(wordId);
  pendingWrite.wordCount -= words.length;
  pendingWrite.textLength -= words.reduce(
    (length, word) => length + word.text.length,
    0,
  );
}

function exceedsSafeBounds(pendingWrite: PendingTranscriptWrite) {
  return (
    pendingWrite.wordCount > MAX_PENDING_WORDS ||
    pendingWrite.partials.length > MAX_PENDING_WORDS ||
    pendingWrite.textLength > MAX_PENDING_TEXT_LENGTH ||
    pendingWrite.replacedRootIds.size > MAX_PENDING_REPLACED_IDS
  );
}
