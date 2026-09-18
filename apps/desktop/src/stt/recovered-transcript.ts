import type { RecoveryInterval } from "./capture-audio-recovery";
import type { WordWithId } from "./types";

export function selectRecoveredWords(
  words: WordWithId[],
  existing: WordWithId[],
  intervals: RecoveryInterval[],
) {
  return words.filter((word) => {
    const start = Number(word.start_ms);
    const end = Number(word.end_ms);
    if (!Number.isFinite(start) || !Number.isFinite(end) || end < start)
      return false;
    return (
      intervals.some(
        (gap) =>
          start < gap.end &&
          (end > gap.start || (start === end && start >= gap.start)),
      ) &&
      !existing.some(
        (saved) =>
          saved.id === word.id ||
          (saved.channel === word.channel &&
            Number(saved.start_ms) < end + 150 &&
            Number(saved.end_ms) > start - 150),
      )
    );
  });
}
