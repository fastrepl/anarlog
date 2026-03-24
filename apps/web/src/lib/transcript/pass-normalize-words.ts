import type { NormalizedWord, SegmentWord, WordLike } from "./shared";

export function normalizeWords(
  finalWords: readonly WordLike[],
  partialWords: readonly WordLike[],
): NormalizedWord[] {
  const combined = [
    ...finalWords.map((word) => toSegmentWord(word, true)),
    ...partialWords.map((word) => toSegmentWord(word, false)),
  ];

  return combined
    .sort((a, b) => a.start_ms - b.start_ms)
    .map((word, order) => ({ ...word, order }));
}

const toSegmentWord = (word: WordLike, isFinal: boolean): SegmentWord => {
  const normalized: SegmentWord = {
    text: word.text,
    start_ms: word.start_ms,
    end_ms: word.end_ms,
    channel: word.channel,
    isFinal,
  };

  if ("id" in word && word.id) {
    normalized.id = word.id as string;
  }

  return normalized;
};
