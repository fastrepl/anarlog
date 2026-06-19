import type { LiveTranscriptDelta } from "@hypr/plugin-transcription";

import type { SpeakerHintWithId, WordWithId } from "./types";

import type { SegmentKey } from "~/stt/live-segment";

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

const dirtyAccumulatorTranscriptIds = new Set<string>();
const activeAccumulatorCounts = new Map<string, number>();
const MAX_SEGMENT_GAP_MS = 3000;

type TranscriptAccumulatorInitialState = {
  words: WordWithId[];
  hints: SpeakerHintWithId[];
};

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
  writeTranscriptHints(store, transcriptId, hints);
  markTranscriptAccumulatorDirty(transcriptId);
}

function writeTranscriptHints(
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

export function createTranscriptAccumulator(
  store: TranscriptStore,
  transcriptId: string,
  initialState?: TranscriptAccumulatorInitialState,
): TranscriptAccumulator {
  return new TranscriptAccumulator(store, transcriptId, initialState);
}

export class TranscriptAccumulator {
  private words: WordWithId[];
  private hints: SpeakerHintWithId[];
  private disposed = false;

  constructor(
    private readonly store: TranscriptStore,
    private readonly transcriptId: string,
    initialState?: TranscriptAccumulatorInitialState,
  ) {
    this.words = initialState
      ? [...initialState.words]
      : parseTranscriptWords(store, transcriptId);
    this.hints = initialState
      ? [...initialState.hints]
      : parseTranscriptHints(store, transcriptId);

    activeAccumulatorCounts.set(
      transcriptId,
      (activeAccumulatorCounts.get(transcriptId) ?? 0) + 1,
    );
  }

  applyLiveDelta(delta: LiveTranscriptDelta): void {
    this.refreshIfDirty();

    const previousWords = this.words;
    const replacedIds = new Set(delta.replaced_ids);
    const newWords: WordWithId[] = delta.new_words.map((word) => ({
      id: word.id,
      text: word.text,
      start_ms: word.start_ms,
      end_ms: word.end_ms,
      channel: word.channel,
    }));
    const newWordIds = new Set(newWords.map((word) => word.id));

    this.words = this.words
      .filter((word) => {
        const wordId = word.id ?? "";
        return !replacedIds.has(wordId) && !newWordIds.has(wordId);
      })
      .concat(newWords)
      .sort((a, b) => (a.start_ms ?? 0) - (b.start_ms ?? 0));

    this.hints = this.hints
      .flatMap((hint) =>
        reconcileSegmentSpeakerAssignmentHint({
          hint,
          replacedIds,
          previousWords,
          nextWords: this.words,
          hints: this.hints,
          newFinalWords: delta.new_words,
        }),
      )
      .filter((hint) => {
        if (isSegmentSpeakerAssignmentHint(hint)) {
          return true;
        }

        const wordId = hint.word_id ?? "";
        return !replacedIds.has(wordId) && !newWordIds.has(wordId);
      })
      .concat(delta.new_words.flatMap(toStorageSpeakerHints))
      .sort((a, b) => (a.word_id ?? "").localeCompare(b.word_id ?? ""));

    this.flush();
  }

  appendWordsAndHints(
    words: WordWithId[],
    hints: SpeakerHintWithId[],
    options?: { mode?: "append" | "replace" },
  ): void {
    if (options?.mode === "replace") {
      this.words = [];
      this.hints = [];
    } else {
      this.refreshIfDirty();
    }

    this.words = this.words.concat(words);
    this.hints = this.hints.concat(hints);
    this.flush();
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }

    this.disposed = true;

    const nextCount = (activeAccumulatorCounts.get(this.transcriptId) ?? 1) - 1;
    if (nextCount > 0) {
      activeAccumulatorCounts.set(this.transcriptId, nextCount);
      return;
    }

    activeAccumulatorCounts.delete(this.transcriptId);
    dirtyAccumulatorTranscriptIds.delete(this.transcriptId);
  }

  private refreshIfDirty(): void {
    if (!dirtyAccumulatorTranscriptIds.delete(this.transcriptId)) {
      return;
    }

    this.words = parseTranscriptWords(this.store, this.transcriptId);
    this.hints = parseTranscriptHints(this.store, this.transcriptId);
  }

  private flush(): void {
    updateTranscriptWords(this.store, this.transcriptId, this.words);
    writeTranscriptHints(this.store, this.transcriptId, this.hints);
  }
}

export function applyLiveTranscriptDelta(
  store: TranscriptStore,
  transcriptId: string,
  delta: LiveTranscriptDelta,
): void {
  const accumulator = createTranscriptAccumulator(store, transcriptId);
  accumulator.applyLiveDelta(delta);
  accumulator.dispose();
}

export function upsertSpeakerAssignment(
  store: TranscriptStore,
  transcriptId: string,
  segmentKey: SegmentKey,
  humanId: string,
  anchorWordId: string,
  options: {
    mode?: "all" | "segment";
    wordIds?: string[];
  } = {},
): void {
  const hints = parseTranscriptHints(store, transcriptId);
  const words = parseTranscriptWords(store, transcriptId);
  const wordsById = new Map(words.map((word) => [word.id, word]));
  const mode = options.mode ?? "all";
  const assignmentWordIds =
    mode === "segment"
      ? getUniqueWordIds([...(options.wordIds ?? []), anchorWordId])
      : [];
  const channel =
    segmentKey.channel === "DirectMic"
      ? 0
      : segmentKey.channel === "RemoteParty"
        ? 1
        : 2;
  const nextScope: SpeakerAssignmentScope =
    mode === "segment"
      ? {
          kind: "words",
          wordIds: new Set(assignmentWordIds),
        }
      : {
          kind: "all",
          channel,
          speakerIndex:
            typeof segmentKey.speaker_index === "number"
              ? segmentKey.speaker_index
              : null,
        };

  const newHint: SpeakerHintWithId = {
    id:
      mode === "segment"
        ? `${anchorWordId}:user_speaker_assignment:segment`
        : `${anchorWordId}:user_speaker_assignment`,
    word_id: anchorWordId,
    type: "user_speaker_assignment",
    value: JSON.stringify(
      mode === "segment"
        ? { human_id: humanId, scope: "segment", word_ids: assignmentWordIds }
        : { human_id: humanId },
    ),
  };

  const nextHints = hints.filter((hint) => {
    if (hint.type !== "user_speaker_assignment") {
      return true;
    }

    if (hint.id === newHint.id) {
      return false;
    }

    const hintScope = getSpeakerAssignmentScopeForHint(hints, wordsById, hint);
    if (!hintScope) {
      return true;
    }

    return !speakerAssignmentScopesConflict(
      hintScope,
      nextScope,
      hints,
      wordsById,
    );
  });

  nextHints.push(newHint);
  updateTranscriptHints(store, transcriptId, nextHints);
}

function markTranscriptAccumulatorDirty(transcriptId: string): void {
  if (activeAccumulatorCounts.has(transcriptId)) {
    dirtyAccumulatorTranscriptIds.add(transcriptId);
  }
}

type SpeakerAssignmentScope =
  | {
      kind: "all";
      channel: number | null | undefined;
      speakerIndex: number | null;
    }
  | {
      kind: "words";
      wordIds: Set<string>;
    };

function getSpeakerAssignmentScopeForHint(
  hints: SpeakerHintWithId[],
  wordsById: Map<string, WordWithId>,
  hint: SpeakerHintWithId,
): SpeakerAssignmentScope | null {
  const value = parseHintValue(hint.value);
  if (
    value &&
    typeof value === "object" &&
    (value as { scope?: unknown }).scope === "segment" &&
    Array.isArray((value as { word_ids?: unknown }).word_ids)
  ) {
    return {
      kind: "words",
      wordIds: new Set(
        (value as { word_ids: unknown[] }).word_ids.filter(
          (wordId): wordId is string =>
            typeof wordId === "string" && wordId.length > 0,
        ),
      ),
    };
  }

  const wordId = hint.word_id;
  if (typeof wordId !== "string") {
    return null;
  }

  const word = wordsById.get(wordId);
  if (!word) {
    return null;
  }

  return {
    kind: "all",
    channel: word.channel,
    speakerIndex: findSpeakerIndexForWord(hints, wordId),
  };
}

function speakerAssignmentScopesConflict(
  left: SpeakerAssignmentScope,
  right: SpeakerAssignmentScope,
  hints: SpeakerHintWithId[],
  wordsById: Map<string, WordWithId>,
): boolean {
  if (right.kind === "words") {
    if (left.kind === "words") {
      return setsOverlap(left.wordIds, right.wordIds);
    }

    return false;
  }

  if (left.kind === "words") {
    for (const wordId of left.wordIds) {
      const word = wordsById.get(wordId);
      if (!word || word.channel !== right.channel) {
        continue;
      }

      const speakerIndex = findSpeakerIndexForWord(hints, wordId);
      if (right.speakerIndex == null || speakerIndex === right.speakerIndex) {
        return true;
      }
    }

    return false;
  }

  if (left.channel !== right.channel) {
    return false;
  }

  return (
    left.speakerIndex == null ||
    right.speakerIndex == null ||
    left.speakerIndex === right.speakerIndex
  );
}

function findSpeakerIndexForWord(
  hints: SpeakerHintWithId[],
  wordId: string,
): number | null {
  const providerHint = hints.find(
    (h) => h.type === "provider_speaker_index" && h.word_id === wordId,
  );
  if (!providerHint) return null;
  try {
    const data =
      typeof providerHint.value === "string"
        ? JSON.parse(providerHint.value)
        : providerHint.value;
    return typeof data.speaker_index === "number" ? data.speaker_index : null;
  } catch {
    return null;
  }
}

function parseHintValue(value: unknown): unknown {
  if (typeof value === "string") {
    try {
      return JSON.parse(value);
    } catch {
      return undefined;
    }
  }

  return value;
}

function reconcileSegmentSpeakerAssignmentHint({
  hint,
  replacedIds,
  previousWords,
  nextWords,
  hints,
  newFinalWords,
}: {
  hint: SpeakerHintWithId;
  replacedIds: Set<string>;
  previousWords: WordWithId[];
  nextWords: WordWithId[];
  hints: SpeakerHintWithId[];
  newFinalWords: LiveTranscriptDelta["new_words"];
}): SpeakerHintWithId[] {
  const segmentAssignment = getSegmentSpeakerAssignment(hint);
  if (!segmentAssignment) {
    return [hint];
  }

  const nextWordIds = getReconciledSegmentWordIds({
    segmentWordIds: segmentAssignment.wordIds,
    replacedIds,
    previousWords,
    nextWords,
    hints,
    newFinalWords,
  });
  const hintWordId = hint.word_id ?? "";
  const nextAnchorWordId = replacedIds.has(hintWordId)
    ? nextWordIds[0]
    : hintWordId;

  if (!nextAnchorWordId || nextWordIds.length === 0) {
    return [];
  }

  return [
    {
      ...hint,
      id: `${nextAnchorWordId}:user_speaker_assignment:segment`,
      word_id: nextAnchorWordId,
      value: JSON.stringify({
        ...segmentAssignment.value,
        word_ids: nextWordIds,
      }),
    },
  ];
}

function getReconciledSegmentWordIds({
  segmentWordIds,
  replacedIds,
  previousWords,
  nextWords,
  hints,
  newFinalWords,
}: {
  segmentWordIds: string[];
  replacedIds: Set<string>;
  previousWords: WordWithId[];
  nextWords: WordWithId[];
  hints: SpeakerHintWithId[];
  newFinalWords: LiveTranscriptDelta["new_words"];
}): string[] {
  const previousWordsById = new Map(
    previousWords.map((word) => [word.id, word]),
  );
  const nextWordsById = new Map(nextWords.map((word) => [word.id, word]));
  const newSpeakerIndexByWordId = new Map(
    newFinalWords.flatMap((word) =>
      typeof word.speaker_index === "number"
        ? [[word.id, word.speaker_index] as const]
        : [],
    ),
  );
  const scopedPreviousWords = segmentWordIds.flatMap((wordId) => {
    const word = previousWordsById.get(wordId);
    return word ? [word] : [];
  });
  const scopedNextWords = segmentWordIds.flatMap((wordId) => {
    const word = nextWordsById.get(wordId);
    return word ? [word] : [];
  });
  const anchorWord = scopedPreviousWords[0] ?? scopedNextWords[0];
  if (!anchorWord) {
    return [];
  }

  const segmentKey = getSpeakerSegmentKey(
    anchorWord,
    hints,
    newSpeakerIndexByWordId,
  );
  const seedWordIds = new Set(
    segmentWordIds.filter((wordId) => {
      return !replacedIds.has(wordId) && nextWordsById.has(wordId);
    }),
  );

  if (segmentWordIds.some((wordId) => replacedIds.has(wordId))) {
    const previousRange = getWordRange(scopedPreviousWords);
    for (const word of newFinalWords) {
      if (
        previousRange &&
        isSameSpeakerSegment(
          word,
          segmentKey,
          hints,
          newSpeakerIndexByWordId,
        ) &&
        isWithinSegmentRange(word, previousRange)
      ) {
        seedWordIds.add(word.id);
      }
    }
  }

  if (seedWordIds.size === 0) {
    return [];
  }

  const seedIndexes = nextWords.flatMap((word, index) =>
    seedWordIds.has(word.id) ? [index] : [],
  );
  if (seedIndexes.length === 0) {
    return [];
  }

  let startIndex = Math.min(...seedIndexes);
  let endIndex = Math.max(...seedIndexes);

  while (
    startIndex > 0 &&
    canMergeSegmentWords(
      nextWords[startIndex - 1],
      nextWords[startIndex],
      segmentKey,
      hints,
      newSpeakerIndexByWordId,
    )
  ) {
    startIndex -= 1;
  }

  while (
    endIndex < nextWords.length - 1 &&
    canMergeSegmentWords(
      nextWords[endIndex],
      nextWords[endIndex + 1],
      segmentKey,
      hints,
      newSpeakerIndexByWordId,
    )
  ) {
    endIndex += 1;
  }

  return getUniqueWordIds(
    nextWords
      .slice(startIndex, endIndex + 1)
      .filter((word) =>
        isSameSpeakerSegment(word, segmentKey, hints, newSpeakerIndexByWordId),
      )
      .map((word) => word.id),
  );
}

function getSpeakerSegmentKey(
  word: WordWithId,
  hints: SpeakerHintWithId[],
  newSpeakerIndexByWordId: Map<string, number>,
): { channel: number; speakerIndex: number | null } {
  return {
    channel: word.channel ?? 0,
    speakerIndex:
      newSpeakerIndexByWordId.get(word.id) ??
      findSpeakerIndexForWord(hints, word.id) ??
      null,
  };
}

function isSameSpeakerSegment(
  word: WordWithId,
  key: { channel: number; speakerIndex: number | null },
  hints: SpeakerHintWithId[],
  newSpeakerIndexByWordId: Map<string, number>,
): boolean {
  const wordKey = getSpeakerSegmentKey(word, hints, newSpeakerIndexByWordId);
  return (
    wordKey.channel === key.channel && wordKey.speakerIndex === key.speakerIndex
  );
}

function canMergeSegmentWords(
  left: WordWithId,
  right: WordWithId,
  key: { channel: number; speakerIndex: number | null },
  hints: SpeakerHintWithId[],
  newSpeakerIndexByWordId: Map<string, number>,
): boolean {
  return (
    isSameSpeakerSegment(left, key, hints, newSpeakerIndexByWordId) &&
    isSameSpeakerSegment(right, key, hints, newSpeakerIndexByWordId) &&
    (right.start_ms ?? 0) - (left.end_ms ?? 0) <= MAX_SEGMENT_GAP_MS
  );
}

function getWordRange(
  words: WordWithId[],
): { startMs: number; endMs: number } | null {
  if (words.length === 0) {
    return null;
  }

  return {
    startMs: Math.min(...words.map((word) => word.start_ms ?? 0)),
    endMs: Math.max(...words.map((word) => word.end_ms ?? 0)),
  };
}

function isWithinSegmentRange(
  word: WordWithId,
  range: { startMs: number; endMs: number },
): boolean {
  return (
    (word.start_ms ?? 0) <= range.endMs + MAX_SEGMENT_GAP_MS &&
    (word.end_ms ?? 0) >= range.startMs - MAX_SEGMENT_GAP_MS
  );
}

function isSegmentSpeakerAssignmentHint(hint: SpeakerHintWithId): boolean {
  return getSegmentSpeakerAssignment(hint) !== null;
}

function getSegmentSpeakerAssignment(
  hint: SpeakerHintWithId,
): { value: Record<string, unknown>; wordIds: string[] } | null {
  if (hint.type !== "user_speaker_assignment") {
    return null;
  }

  const value = parseHintValue(hint.value);
  if (
    !value ||
    typeof value !== "object" ||
    (value as { scope?: unknown }).scope !== "segment" ||
    !Array.isArray((value as { word_ids?: unknown }).word_ids)
  ) {
    return null;
  }

  return {
    value: value as Record<string, unknown>,
    wordIds: getUniqueWordIds((value as { word_ids: unknown[] }).word_ids),
  };
}

function getUniqueWordIds(wordIds: unknown[]): string[] {
  return Array.from(
    new Set(
      wordIds.filter(
        (wordId): wordId is string =>
          typeof wordId === "string" && wordId.length > 0,
      ),
    ),
  );
}

function setsOverlap(left: Set<string>, right: Set<string>): boolean {
  for (const value of left) {
    if (right.has(value)) {
      return true;
    }
  }

  return false;
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
