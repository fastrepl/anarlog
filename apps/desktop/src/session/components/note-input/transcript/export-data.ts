import * as main from "~/store/tinybase/store/main";
import { buildSegments, SegmentKey } from "~/stt/segment";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";
import { convertStorageHintsToRuntime } from "~/stt/speaker-hints";
import { parseTranscriptHints, parseTranscriptWords } from "~/stt/utils";

export function buildTranscriptExportSegments(
  store: NonNullable<ReturnType<typeof main.UI.useStore>>,
  transcriptIds: string[],
) {
  if (transcriptIds.length === 0) {
    return [];
  }

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
    transcriptIds[0],
    "started_at",
  );

  for (const transcriptId of transcriptIds) {
    const startedAt = store.getCell("transcripts", transcriptId, "started_at");
    const offset =
      typeof startedAt === "number" && typeof firstStartedAt === "number"
        ? startedAt - firstStartedAt
        : 0;

    const words = parseTranscriptWords(store, transcriptId);
    for (const word of words) {
      if (
        word.text === undefined ||
        word.start_ms === undefined ||
        word.end_ms === undefined
      ) {
        continue;
      }

      collectedWords.push({
        id: word.id,
        text: word.text,
        start_ms: word.start_ms + offset,
        end_ms: word.end_ms + offset,
        channel: word.channel ?? 0,
      });
    }
  }

  collectedWords.sort((a, b) => a.start_ms - b.start_ms);
  collectedWords.forEach((word, index) => wordIdToIndex.set(word.id, index));

  const storageHints = transcriptIds.flatMap((transcriptId) =>
    parseTranscriptHints(store, transcriptId),
  );
  const speakerHints = convertStorageHintsToRuntime(
    storageHints,
    wordIdToIndex,
  );

  const segments = buildSegments(collectedWords, [], speakerHints);
  const ctx = defaultRenderLabelContext(store);
  const manager = SpeakerLabelManager.fromSegments(segments, ctx);

  return segments.flatMap((segment) => {
    if (segment.words.length === 0) {
      return [];
    }

    const text = segment.words
      .map((word) => word.text)
      .join(" ")
      .trim();
    if (!text) {
      return [];
    }

    const firstWord = segment.words[0];
    const lastWord = segment.words[segment.words.length - 1];

    return [
      {
        text,
        start_ms: firstWord.start_ms,
        end_ms: lastWord.end_ms,
        speaker: SegmentKey.renderLabel(segment.key, ctx, manager),
      },
    ];
  });
}

export function formatTranscriptExportSegments(
  segments: Array<{ speaker: string; text: string }>,
) {
  return segments
    .map((segment) => `${segment.speaker}: ${segment.text}`)
    .join("\n\n");
}
