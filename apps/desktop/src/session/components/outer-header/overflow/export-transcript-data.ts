import type { TranscriptItem } from "@hypr/plugin-export";
import type { VttWord } from "@hypr/plugin-listener2";

import type { Store } from "~/store/tinybase/store/main";
import { buildSegments, SegmentKey } from "~/stt/segment";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";
import { convertStorageHintsToRuntime } from "~/stt/speaker-hints";
import { parseTranscriptHints, parseTranscriptWords } from "~/stt/utils";

type ExportStore = Pick<Store, "getCell" | "getRow" | "getValue">;

export function formatTranscriptDuration(
  startMs: number,
  endMs: number,
): string {
  const durationMs = endMs - startMs;
  const minutes = Math.floor(durationMs / 60000);
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;

  if (hours > 0) {
    return `${hours}h ${remainingMinutes}m`;
  }

  return `${minutes}m`;
}

export function getTranscriptExportData(
  store: ExportStore,
  transcriptIds: string[],
): {
  duration: string | null;
  items: TranscriptItem[];
  vttWords: VttWord[];
} {
  if (transcriptIds.length === 0) {
    return {
      duration: null,
      items: [],
      vttWords: [],
    };
  }

  const orderedTranscriptIds = [...transcriptIds].sort((a, b) => {
    const aStartedAt = store.getCell("transcripts", a, "started_at");
    const bStartedAt = store.getCell("transcripts", b, "started_at");

    const aValue =
      typeof aStartedAt === "number" ? aStartedAt : Number.MAX_SAFE_INTEGER;
    const bValue =
      typeof bStartedAt === "number" ? bStartedAt : Number.MAX_SAFE_INTEGER;

    return aValue - bValue;
  });

  const wordIdToIndex = new Map<string, number>();
  const collectedWords: Array<{
    id: string;
    text: string;
    start_ms: number;
    end_ms: number;
    channel: number;
  }> = [];

  const firstStartedAt = store.getCell(
    "transcripts",
    orderedTranscriptIds[0],
    "started_at",
  );

  let minStartedAt: number | null = null;
  let maxEndedAt: number | null = null;

  for (const transcriptId of orderedTranscriptIds) {
    const startedAt = store.getCell("transcripts", transcriptId, "started_at");
    const endedAt = store.getCell("transcripts", transcriptId, "ended_at");

    if (typeof startedAt === "number") {
      if (minStartedAt === null || startedAt < minStartedAt) {
        minStartedAt = startedAt;
      }
    }

    if (typeof endedAt === "number") {
      if (maxEndedAt === null || endedAt > maxEndedAt) {
        maxEndedAt = endedAt;
      }
    }

    const offset =
      typeof startedAt === "number" && typeof firstStartedAt === "number"
        ? startedAt - firstStartedAt
        : 0;

    const words = parseTranscriptWords(store, transcriptId);
    for (const word of words) {
      if (word.text === undefined || word.start_ms === undefined) {
        continue;
      }

      collectedWords.push({
        id: word.id,
        text: word.text,
        start_ms: word.start_ms + offset,
        end_ms: (word.end_ms ?? word.start_ms) + offset,
        channel: word.channel ?? 0,
      });
    }
  }

  if (collectedWords.length === 0) {
    return {
      duration:
        minStartedAt !== null && maxEndedAt !== null
          ? formatTranscriptDuration(minStartedAt, maxEndedAt)
          : null,
      items: [],
      vttWords: [],
    };
  }

  collectedWords.sort((a, b) => a.start_ms - b.start_ms);
  collectedWords.forEach((word, index) => wordIdToIndex.set(word.id, index));

  const storageHints = orderedTranscriptIds.flatMap((id) =>
    parseTranscriptHints(store, id),
  );
  const speakerHints = convertStorageHintsToRuntime(
    storageHints,
    wordIdToIndex,
  );

  const segments = buildSegments(collectedWords, [], speakerHints);
  const ctx = defaultRenderLabelContext(store);
  const manager = SpeakerLabelManager.fromSegments(segments, ctx);

  return {
    duration:
      minStartedAt !== null && maxEndedAt !== null
        ? formatTranscriptDuration(minStartedAt, maxEndedAt)
        : null,
    items: segments
      .filter((segment) => segment.words.length > 0)
      .map((segment) => ({
        speaker: SegmentKey.renderLabel(segment.key, ctx, manager),
        text: segment.words.map((word) => word.text).join(" "),
      })),
    vttWords: segments
      .filter((segment) => segment.words.length > 0)
      .map((segment) => {
        const firstWord = segment.words[0];
        const lastWord = segment.words[segment.words.length - 1];

        return {
          text: segment.words.map((word) => word.text).join(" "),
          start_ms: firstWord.start_ms,
          end_ms: lastWord.end_ms,
          speaker: SegmentKey.renderLabel(segment.key, ctx, manager),
        };
      }),
  };
}
