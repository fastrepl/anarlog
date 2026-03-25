import type { LiveTranscriptDelta } from "@hypr/plugin-listener";

import type { SpeakerHintWithId, WordWithId } from "./types";

interface TranscriptStore {
  getCell(
    tableId: "transcripts",
    rowId: string,
    cellId: "words" | "speaker_hints",
  ): unknown;
  setCell(
    tableId: "transcripts",
    rowId: string,
    cellId: "words" | "speaker_hints",
    value: string,
  ): void;
}

export function parseTranscriptWords(
  store: TranscriptStore,
  transcriptId: string,
): WordWithId[] {
  const wordsJson = store.getCell("transcripts", transcriptId, "words");
  if (typeof wordsJson !== "string" || !wordsJson) {
    return [];
  }

  try {
    return JSON.parse(wordsJson) as WordWithId[];
  } catch {
    return [];
  }
}

export function parseTranscriptHints(
  store: TranscriptStore,
  transcriptId: string,
): SpeakerHintWithId[] {
  const hintsJson = store.getCell("transcripts", transcriptId, "speaker_hints");
  if (typeof hintsJson !== "string" || !hintsJson) {
    return [];
  }

  try {
    return JSON.parse(hintsJson) as SpeakerHintWithId[];
  } catch {
    return [];
  }
}

export function updateTranscriptWords(
  store: TranscriptStore,
  transcriptId: string,
  words: WordWithId[],
): void {
  store.setCell("transcripts", transcriptId, "words", JSON.stringify(words));
}

export function updateTranscriptHints(
  store: TranscriptStore,
  transcriptId: string,
  hints: SpeakerHintWithId[],
): void {
  store.setCell(
    "transcripts",
    transcriptId,
    "speaker_hints",
    JSON.stringify(hints),
  );
}

export function applyLiveTranscriptDelta(
  store: TranscriptStore,
  transcriptId: string,
  delta: LiveTranscriptDelta,
): void {
  const existingWords = parseTranscriptWords(store, transcriptId);
  const existingHints = parseTranscriptHints(store, transcriptId);

  const replacedIds = new Set(delta.replaced_ids);
  const newWords: WordWithId[] = delta.new_words.map((word) => ({
    id: word.id,
    text: word.text,
    start_ms: word.start_ms,
    end_ms: word.end_ms,
    channel: word.channel,
  }));
  const newWordIds = new Set(newWords.map((word) => word.id));

  const nextWords = existingWords
    .filter((word) => {
      const wordId = word.id ?? "";
      return !replacedIds.has(wordId) && !newWordIds.has(wordId);
    })
    .concat(newWords)
    .sort((a, b) => (a.start_ms ?? 0) - (b.start_ms ?? 0));

  const nextHints = existingHints
    .filter((hint) => {
      const wordId = hint.word_id ?? "";
      return !replacedIds.has(wordId) && !newWordIds.has(wordId);
    })
    .concat(delta.new_words.flatMap(toStorageSpeakerHints))
    .sort((a, b) => (a.word_id ?? "").localeCompare(b.word_id ?? ""));

  updateTranscriptWords(store, transcriptId, nextWords);
  updateTranscriptHints(store, transcriptId, nextHints);
}

function toStorageSpeakerHints(
  word: LiveTranscriptDelta["new_words"][number],
): SpeakerHintWithId[] {
  if (word.speaker_index == null) {
    return [];
  }

  return [
    {
      id: `${word.id}:provider_speaker_index`,
      word_id: word.id,
      type: "provider_speaker_index",
      value: JSON.stringify({
        channel: word.channel,
        speaker_index: word.speaker_index,
      }),
    },
  ];
}
