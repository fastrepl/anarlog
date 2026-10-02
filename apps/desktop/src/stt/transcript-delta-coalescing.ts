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
  wordCount: number;
  textLength: number;
};

function createPendingWrite(): PendingTranscriptWrite {
  return {
    wordsById: new Map(),
    replacedRootIds: new Set(),
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
      mayExistInPersistedBase:
        existing?.mayExistInPersistedBase ?? delta.replaced_ids.length === 0,
    });
    pendingWrite.wordCount += words.length;
    pendingWrite.textLength += words.reduce(
      (length, word) => length + word.text.length,
      0,
    );
  }
}

function toDelta(pendingWrite: PendingTranscriptWrite): LiveTranscriptDelta {
  const newWords: LiveTranscriptDelta["new_words"] = [];
  for (const { words } of pendingWrite.wordsById.values()) {
    newWords.push(...words);
  }

  return {
    new_words: newWords,
    replaced_ids: [...pendingWrite.replacedRootIds],
    partials: [],
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
