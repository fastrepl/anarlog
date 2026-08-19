import type { Segment, SegmentKey } from "~/stt/live-segment";

export type TranscriptWordSelectionGroup = {
  transcriptId: string;
  segmentKey: SegmentKey;
  wordIds: string[];
};

export type TranscriptWordSelection = {
  sessionId?: string;
  text: string;
  startMs: number;
  groups: TranscriptWordSelectionGroup[];
};

export function getTranscriptSelectionFromRange(
  range: Range,
  container: HTMLElement,
): TranscriptWordSelection | null {
  if (range.collapsed || !container.contains(range.commonAncestorContainer)) {
    return null;
  }

  const groups = new Map<string, TranscriptWordSelectionGroup>();
  let sessionId: string | undefined;
  let startMs: number | undefined;

  for (const wordElement of container.querySelectorAll<HTMLElement>(
    "[data-transcript-word-id]",
  )) {
    if (!rangeIntersectsNode(range, wordElement)) {
      continue;
    }

    const wordId = wordElement.dataset.transcriptWordId;
    const segmentElement = wordElement.closest<HTMLElement>(
      "[data-transcript-id]",
    );
    const transcriptId = segmentElement?.dataset.transcriptId;
    const segmentKey = segmentElement
      ? readSegmentKey(segmentElement)
      : undefined;
    if (!wordId || !transcriptId || !segmentKey) {
      continue;
    }

    addWordToSelection(groups, transcriptId, segmentKey, wordId);
    sessionId ??= segmentElement?.dataset.sessionId || undefined;
    startMs ??= readWordStartMs(wordElement, segmentElement);
  }

  for (const editorElement of container.querySelectorAll<HTMLElement>(
    "[data-transcript-edit-word-ids]",
  )) {
    if (!rangeIntersectsNode(range, editorElement)) {
      continue;
    }

    const segmentElement = editorElement.closest<HTMLElement>(
      "[data-transcript-id]",
    );
    const transcriptId = segmentElement?.dataset.transcriptId;
    const segmentKey = segmentElement
      ? readSegmentKey(segmentElement)
      : undefined;
    if (!transcriptId || !segmentKey) {
      continue;
    }

    const wordIds = parseStringArray(
      editorElement.dataset.transcriptEditWordIds,
    );
    const wordTexts = parseStringArray(
      editorElement.dataset.transcriptEditWordTexts,
    );
    const wordStartMs = parseNumberArray(
      editorElement.dataset.transcriptEditWordStartMs,
    );
    const selectedIndices = getSelectedEditableWordIndices(
      range,
      editorElement,
      wordTexts,
    );
    for (const index of selectedIndices) {
      const wordId = wordIds[index];
      if (!wordId) {
        continue;
      }
      addWordToSelection(groups, transcriptId, segmentKey, wordId);
      sessionId ??= segmentElement?.dataset.sessionId || undefined;
      startMs ??=
        (wordStartMs[index] ?? 0) +
        Number(segmentElement?.dataset.transcriptOffsetMs ?? 0);
    }
  }

  if (groups.size === 0) {
    return null;
  }

  return {
    sessionId,
    text: range.toString().trim(),
    startMs: startMs ?? 0,
    groups: [...groups.values()],
  };
}

export function getTranscriptContextSelection({
  target,
  container,
  activeRange,
}: {
  target: EventTarget | null;
  container: HTMLElement;
  activeRange?: Range;
}): { range: Range; selection: TranscriptWordSelection } | null {
  if (!(target instanceof Element)) {
    return null;
  }

  const selectionTarget = target.closest<HTMLElement>(
    "[data-transcript-word-id], [data-transcript-editor]",
  );
  if (!selectionTarget || !container.contains(selectionTarget)) {
    return null;
  }

  if (
    activeRange &&
    !activeRange.collapsed &&
    rangeIntersectsNode(activeRange, selectionTarget)
  ) {
    const selection = getTranscriptSelectionFromRange(activeRange, container);
    if (selection) {
      return { range: activeRange.cloneRange(), selection };
    }
  }

  const segmentContent = selectionTarget.closest<HTMLElement>(
    "[data-transcript-segment-content]",
  );
  if (!segmentContent) {
    return null;
  }

  const range = document.createRange();
  range.selectNodeContents(segmentContent);
  const selection = getTranscriptSelectionFromRange(range, container);
  return selection ? { range, selection } : null;
}

export function getTranscriptSegmentDomId(
  transcriptId: string,
  segment: Pick<Segment, "id" | "start_ms" | "end_ms" | "words">,
) {
  return (
    segment.id ||
    `${transcriptId}:${segment.words[0]?.id ?? segment.start_ms}:${segment.words[segment.words.length - 1]?.id ?? segment.end_ms}`
  );
}

export function getTranscriptSectionKey(
  transcriptId: string,
  segmentId: string,
) {
  return `${transcriptId}:${segmentId}`;
}

export function getTranscriptSectionKeyFromElement(section: HTMLElement) {
  const transcriptId = section.dataset.transcriptId;
  const segmentId = section.dataset.transcriptSegmentId;
  return transcriptId && segmentId
    ? getTranscriptSectionKey(transcriptId, segmentId)
    : null;
}

export function getTranscriptSelectionFromSegment({
  transcriptId,
  sessionId,
  offsetMs,
  segment,
}: {
  transcriptId: string;
  sessionId?: string;
  offsetMs: number;
  segment: Pick<Segment, "key" | "text" | "words">;
}): TranscriptWordSelection | null {
  const wordIds = segment.words
    .map((word) => word.id)
    .filter(
      (wordId): wordId is string =>
        typeof wordId === "string" && wordId.length > 0,
    );
  if (wordIds.length === 0) {
    return null;
  }

  return {
    sessionId,
    text: segment.text.trim(),
    startMs: offsetMs + (segment.words[0]?.start_ms ?? 0),
    groups: [
      {
        transcriptId,
        segmentKey: segment.key,
        wordIds,
      },
    ],
  };
}

export function getTranscriptSectionSelection(
  section: HTMLElement,
  container: HTMLElement,
): TranscriptWordSelection | null {
  const content = section.querySelector<HTMLElement>(
    "[data-transcript-segment-content]",
  );
  if (!content) {
    return null;
  }

  const range = document.createRange();
  range.selectNodeContents(content);
  return getTranscriptSelectionFromRange(range, container);
}

export function mergeTranscriptSelections(
  selections: TranscriptWordSelection[],
): TranscriptWordSelection | null {
  if (selections.length === 0) {
    return null;
  }

  const groups = new Map<string, TranscriptWordSelectionGroup>();
  for (const selection of selections) {
    for (const nextGroup of selection.groups) {
      const group = groups.get(nextGroup.transcriptId) ?? {
        transcriptId: nextGroup.transcriptId,
        segmentKey: nextGroup.segmentKey,
        wordIds: [],
      };
      for (const wordId of nextGroup.wordIds) {
        if (!group.wordIds.includes(wordId)) {
          group.wordIds.push(wordId);
        }
      }
      groups.set(nextGroup.transcriptId, group);
    }
  }

  return {
    sessionId: selections.find((selection) => selection.sessionId)?.sessionId,
    text: selections.map((selection) => selection.text).join("\n\n"),
    startMs: Math.min(...selections.map((selection) => selection.startMs)),
    groups: [...groups.values()],
  };
}

const LIVE_TRANSCRIPT_PLACEHOLDER_ID = "__live-transcript__";

export function canMergeTranscriptEntries(
  selectedKeys: ReadonlySet<string>,
  order: string[],
  entries: Map<string, TranscriptWordSelection>,
) {
  return getTranscriptMergeTarget(selectedKeys, order, entries) != null;
}

export function getTranscriptMergeTarget(
  selectedKeys: ReadonlySet<string>,
  order: string[],
  entries: Map<string, TranscriptWordSelection>,
) {
  if (selectedKeys.size < 2) {
    return null;
  }

  const indexes = order.flatMap((key, index) =>
    selectedKeys.has(key) ? [index] : [],
  );
  if (indexes.length !== selectedKeys.size) {
    return null;
  }

  for (let index = 1; index < indexes.length; index += 1) {
    if (indexes[index] !== indexes[index - 1]! + 1) {
      return null;
    }
  }

  const firstKey = order[indexes[0]!];
  const first = firstKey ? (entries.get(firstKey) ?? null) : null;
  const targetGroup = first?.groups[0];
  if (!first || !targetGroup) {
    return null;
  }
  if (targetGroup.transcriptId === LIVE_TRANSCRIPT_PLACEHOLDER_ID) {
    return null;
  }
  if (
    !targetGroup.segmentKey.speaker_human_id &&
    typeof targetGroup.segmentKey.speaker_index !== "number"
  ) {
    return null;
  }

  for (const index of indexes) {
    const group = entries.get(order[index]!)?.groups[0];
    if (
      !group ||
      group.transcriptId !== targetGroup.transcriptId ||
      group.segmentKey.channel !== targetGroup.segmentKey.channel
    ) {
      return null;
    }
  }

  return first;
}

function readSegmentKey(element: HTMLElement): SegmentKey | undefined {
  const channel = element.dataset.segmentChannel;
  if (
    channel !== "DirectMic" &&
    channel !== "RemoteParty" &&
    channel !== "MixedCapture"
  ) {
    return undefined;
  }

  const speakerIndexValue = element.dataset.segmentSpeakerIndex;
  const speakerIndex =
    speakerIndexValue === undefined || speakerIndexValue === ""
      ? null
      : Number(speakerIndexValue);

  return {
    channel,
    speaker_index: Number.isFinite(speakerIndex) ? speakerIndex : null,
    speaker_human_id: element.dataset.segmentSpeakerHumanId || null,
  };
}

function addWordToSelection(
  groups: Map<string, TranscriptWordSelectionGroup>,
  transcriptId: string,
  segmentKey: SegmentKey,
  wordId: string,
) {
  const group = groups.get(transcriptId) ?? {
    transcriptId,
    segmentKey,
    wordIds: [],
  };
  if (!group.wordIds.includes(wordId)) {
    group.wordIds.push(wordId);
  }
  groups.set(transcriptId, group);
}

function getSelectedEditableWordIndices(
  range: Range,
  editor: HTMLElement,
  wordTexts: string[],
) {
  const offsets = getRangeOffsets(range, editor);
  if (!offsets) {
    return [];
  }

  const indices: number[] = [];
  let cursor = 0;
  for (const [index, wordText] of wordTexts.entries()) {
    const start = cursor;
    const end = start + wordText.length;
    if (offsets.start < end && offsets.end > start) {
      indices.push(index);
    }
    cursor = end + 1;
  }
  return indices;
}

function getRangeOffsets(range: Range, container: HTMLElement) {
  if (
    !container.contains(range.startContainer) ||
    !container.contains(range.endContainer)
  ) {
    return null;
  }

  const beforeStart = document.createRange();
  beforeStart.selectNodeContents(container);
  beforeStart.setEnd(range.startContainer, range.startOffset);
  const beforeEnd = document.createRange();
  beforeEnd.selectNodeContents(container);
  beforeEnd.setEnd(range.endContainer, range.endOffset);
  return {
    start: beforeStart.toString().length,
    end: beforeEnd.toString().length,
  };
}

function parseStringArray(value: string | undefined): string[] {
  try {
    const parsed = JSON.parse(value ?? "[]");
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}

function parseNumberArray(value: string | undefined): number[] {
  try {
    const parsed = JSON.parse(value ?? "[]");
    return Array.isArray(parsed)
      ? parsed.map((item) => (typeof item === "number" ? item : 0))
      : [];
  } catch {
    return [];
  }
}

function readWordStartMs(
  wordElement: HTMLElement,
  segmentElement: HTMLElement | null,
) {
  const wordStartMs = Number(wordElement.dataset.transcriptWordStartMs ?? 0);
  const transcriptOffsetMs = Number(
    segmentElement?.dataset.transcriptOffsetMs ?? 0,
  );
  return wordStartMs + transcriptOffsetMs;
}

function rangeIntersectsNode(range: Range, node: Node) {
  try {
    return range.intersectsNode(node);
  } catch {
    return false;
  }
}
