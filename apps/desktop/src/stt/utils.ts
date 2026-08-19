import type { LiveTranscriptDelta } from "@anlg/plugin-transcription";

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

    const nextWords = this.words.filter((word) => {
      const wordId = word.id ?? "";
      return !replacedIds.has(wordId) && !newWordIds.has(wordId);
    });
    for (const word of newWords) {
      nextWords.push(word);
    }
    nextWords.sort((a, b) => (a.start_ms ?? 0) - (b.start_ms ?? 0));
    this.words = nextWords;

    const nextHints: SpeakerHintWithId[] = [];
    for (const hint of this.hints) {
      const reconciledHints = reconcileSegmentSpeakerAssignmentHint({
        hint,
        replacedIds,
        previousWords,
        nextWords: this.words,
        hints: this.hints,
        newFinalWords: delta.new_words,
      });

      for (const reconciledHint of reconciledHints) {
        if (
          isSegmentSpeakerAssignmentHint(reconciledHint) ||
          isSpeakerScopedAssignmentHint(reconciledHint)
        ) {
          nextHints.push(reconciledHint);
          continue;
        }

        const wordId = reconciledHint.word_id ?? "";
        if (!replacedIds.has(wordId) && !newWordIds.has(wordId)) {
          nextHints.push(reconciledHint);
        }
      }
    }

    for (const word of delta.new_words) {
      for (const hint of toStorageSpeakerHints(word)) {
        nextHints.push(hint);
      }
    }
    nextHints.sort((a, b) => (a.word_id ?? "").localeCompare(b.word_id ?? ""));
    this.hints = nextHints;

    this.flush();
  }

  appendWordsAndHints(
    words: WordWithId[],
    hints: SpeakerHintWithId[],
    options?: { mode?: "append" | "replace" },
  ): void {
    if (options?.mode === "replace") {
      this.words = [...words];
      this.hints = [...hints];
    } else {
      this.refreshIfDirty();
      for (const word of words) {
        this.words.push(word);
      }
      for (const hint of hints) {
        this.hints.push(hint);
      }
    }
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
        : {
            human_id: humanId,
            scope: "speaker",
            channel,
            speaker_index: segmentKey.speaker_index ?? null,
          },
    ),
  };

  const nextHints = hints.filter((hint) => {
    if (
      hint.type !== "automatic_speaker_assignment" &&
      hint.type !== "user_speaker_assignment"
    ) {
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

export function mergeTranscriptSegmentAssignments(
  store: TranscriptStore,
  transcriptId: string,
  segmentKey: SegmentKey,
  wordIds: string[],
): void {
  const assignmentWordIds = getUniqueWordIds(wordIds);
  if (assignmentWordIds.length === 0) {
    return;
  }

  if (segmentKey.speaker_human_id) {
    upsertSpeakerAssignment(
      store,
      transcriptId,
      segmentKey,
      segmentKey.speaker_human_id,
      assignmentWordIds[0]!,
      { mode: "segment", wordIds: assignmentWordIds },
    );
    return;
  }

  if (typeof segmentKey.speaker_index === "number") {
    unifyProviderSpeakerIndex(
      store,
      transcriptId,
      assignmentWordIds,
      segmentKey.speaker_index,
    );
  }
}

function unifyProviderSpeakerIndex(
  store: TranscriptStore,
  transcriptId: string,
  wordIds: string[],
  speakerIndex: number,
): void {
  const wordIdSet = new Set(wordIds);
  const words = parseTranscriptWords(store, transcriptId);
  const wordsById = new Map(words.map((word) => [word.id, word]));
  const hints = parseTranscriptHints(store, transcriptId);
  const nextHints: SpeakerHintWithId[] = [];
  const updatedProviderWordIds = new Set<string>();

  for (const hint of hints) {
    if (
      hint.type === "provider_speaker_index" &&
      typeof hint.word_id === "string" &&
      wordIdSet.has(hint.word_id)
    ) {
      nextHints.push({
        ...hint,
        value: JSON.stringify({
          channel: getProviderHintChannel(hint, wordsById.get(hint.word_id)),
          speaker_index: speakerIndex,
        }),
      });
      updatedProviderWordIds.add(hint.word_id);
      continue;
    }

    if (
      hint.type === "automatic_speaker_assignment" ||
      hint.type === "user_speaker_assignment"
    ) {
      const hintScope = getSpeakerAssignmentScopeForHint(
        hints,
        wordsById,
        hint,
      );
      if (hintScope?.kind === "all") {
        nextHints.push(hint);
        continue;
      }
      if (
        hintScope?.kind === "words" &&
        setsOverlap(hintScope.wordIds, wordIdSet)
      ) {
        continue;
      }
      if (typeof hint.word_id === "string" && wordIdSet.has(hint.word_id)) {
        continue;
      }
    }

    nextHints.push(hint);
  }

  for (const wordId of wordIds) {
    if (updatedProviderWordIds.has(wordId)) {
      continue;
    }
    const word = wordsById.get(wordId);
    if (!word) {
      continue;
    }
    nextHints.push({
      id: `${wordId}:provider_speaker_index`,
      word_id: wordId,
      type: "provider_speaker_index",
      value: JSON.stringify({
        channel: word.channel,
        speaker_index: speakerIndex,
      }),
    });
  }

  updateTranscriptHints(store, transcriptId, nextHints);
}

function getProviderHintChannel(
  hint: SpeakerHintWithId,
  word: WordWithId | undefined,
) {
  const value = parseHintValue(hint.value);
  if (
    value &&
    typeof value === "object" &&
    typeof (value as { channel?: unknown }).channel === "number"
  ) {
    return (value as { channel: number }).channel;
  }

  return word?.channel ?? 0;
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
  const explicitScope =
    value && typeof value === "object"
      ? (value as {
          scope?: unknown;
          channel?: unknown;
          speaker_index?: unknown;
        })
      : null;
  if (
    explicitScope?.scope === "speaker" &&
    (explicitScope.channel === 0 ||
      explicitScope.channel === 1 ||
      explicitScope.channel === 2)
  ) {
    const speakerIndex = explicitScope.speaker_index;
    if (speakerIndex === null || typeof speakerIndex === "number") {
      return {
        kind: "all",
        channel: explicitScope.channel,
        speakerIndex,
      };
    }
  }

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

function isSpeakerScopedAssignmentHint(hint: SpeakerHintWithId): boolean {
  if (
    hint.type !== "automatic_speaker_assignment" &&
    hint.type !== "user_speaker_assignment"
  ) {
    return false;
  }

  const value = parseHintValue(hint.value);
  if (!value || typeof value !== "object") {
    return false;
  }

  const scope = value as {
    scope?: unknown;
    channel?: unknown;
    speaker_index?: unknown;
  };
  return (
    scope.scope === "speaker" &&
    (scope.channel === 0 || scope.channel === 1 || scope.channel === 2) &&
    (scope.speaker_index === null || typeof scope.speaker_index === "number")
  );
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
