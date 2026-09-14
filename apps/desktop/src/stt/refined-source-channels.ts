import type { SpeakerHintWithId, WordWithId } from "~/stt/types";

// A downmixed provider loses source ownership. Recover it only when both the text and timing
// agree with a single original source; overlapping or rewritten speech stays mixed.
export function restoreRefinedSourceChannels(
  previous: WordWithId[],
  replacement: WordWithId[],
): WordWithId[] {
  const original = previous
    .filter((word) => word.channel === 0 || word.channel === 1)
    .sort((a, b) => (a.start_ms ?? 0) - (b.start_ms ?? 0));
  const normalize = (text: string | undefined) =>
    (text ?? "")
      .trim()
      .toLowerCase()
      .replace(/[\p{P}\p{Z}]/gu, "");
  let cursor = 0;
  let lastStart = -Infinity;
  return replacement.map((word) => {
    if (word.channel !== 2) return word;
    const start = word.start_ms ?? 0;
    const end = word.end_ms ?? start;
    const text = normalize(word.text);
    if (!text || end <= start) return word;
    if (start < lastStart) cursor = 0;
    lastStart = start;
    while (cursor < original.length && (original[cursor]!.end_ms ?? 0) <= start)
      cursor++;
    const channels = new Set<number>();
    let matched = false;
    for (let index = cursor; index < original.length; index++) {
      const source = original[index]!;
      const sourceStart = source.start_ms ?? 0;
      const sourceEnd = source.end_ms ?? sourceStart;
      if (sourceStart >= end) break;
      const overlap = Math.min(end, sourceEnd) - Math.max(start, sourceStart);
      if (overlap <= 0) continue;
      channels.add(source.channel!);
      if (
        normalize(source.text) === text &&
        overlap >= 0.8 * Math.max(end - start, sourceEnd - sourceStart)
      )
        matched = true;
    }
    return matched && channels.size === 1
      ? { ...word, channel: [...channels][0]! }
      : word;
  });
}

export function restoreRefinedSourceHints(
  words: WordWithId[],
  hints: SpeakerHintWithId[],
): SpeakerHintWithId[] {
  const channels = new Map(words.map((word) => [word.id, word.channel]));
  return hints.map((hint) => {
    const channel = channels.get(hint.word_id ?? "");
    if (
      hint.type !== "provider_speaker_index" ||
      (channel !== 0 && channel !== 1)
    )
      return hint;
    try {
      const value = JSON.parse(hint.value ?? "null");
      if (value?.channel !== 2) return hint;
      return { ...hint, value: JSON.stringify({ ...value, channel }) };
    } catch {
      return hint;
    }
  });
}
