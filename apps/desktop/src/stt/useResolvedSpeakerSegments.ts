import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import {
  commands,
  type RenderTranscriptRequest,
  type RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

import {
  type Segment,
  type SegmentKey,
  SegmentKeyUtils,
  type SegmentWord,
} from "~/stt/live-segment";

type SpeakerResolution = Pick<
  RenderedTranscriptSegment,
  "speaker_label" | "provisional_speaker"
>;

export function useResolvedSpeakerSegments(
  segments: Segment[],
  request: RenderTranscriptRequest | null,
): Segment[] {
  const input = useMemo(
    () =>
      request?.speaker_context && segments.length
        ? {
            ...request,
            transcripts: request.transcripts.map((transcript) => ({
              started_at: transcript.started_at,
              words: [],
              assignments: [],
            })),
            preview: segments.map((segment) => ({
              ...segment,
              speaker_label: "",
              provisional_speaker: undefined,
            })),
          }
        : null,
    [request, segments],
  );
  // Live captures change the input several times a second; keep the last
  // resolution's speaker names on screen until the native labeler answers for
  // the new one, but always render the current words and segments.
  const { data, isPlaceholderData } = useQuery({
    queryKey: ["contextual-speaker-segments", input],
    queryFn: async () => {
      if (!input) return [];
      const result = await commands.renderTranscriptSegments(input);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    enabled: Boolean(input),
    placeholderData: keepPreviousData,
    staleTime: Infinity,
    gcTime: 0,
  });
  const resolved = useMemo(() => {
    if (!input || !data) return undefined;
    if (isPlaceholderData) {
      return carrySpeakerResolution(segments, data, input);
    }
    const metadata = new Map(
      segments.flatMap((segment) =>
        segment.words.map((word) => [word.id, word.metadata] as const),
      ),
    );
    return data.map((segment) => ({
      ...segment,
      words: segment.words.map((word) => ({
        ...word,
        metadata: metadata.get(word.id),
      })),
    }));
  }, [data, input, isPlaceholderData, segments]);
  return input
    ? (resolved ??
        segments.map((segment) => ({
          ...segment,
          speaker_label: undefined,
          provisional_speaker: undefined,
        })))
    : segments;
}

function carrySpeakerResolution(
  segments: Segment[],
  previous: RenderedTranscriptSegment[],
  request: RenderTranscriptRequest,
): Segment[] {
  // The native labeler names a speaker key per speaker-context interval and
  // splits a segment whose words straddle a boundary, so carried names are
  // scoped the same way and runs are emitted with the labeler's ids.
  const intervalAt = createIntervalLookup(request);
  const scopedKey = (key: SegmentKey, word: SegmentWord) =>
    `${SegmentKeyUtils.serialize(key)}:${
      key.speaker_human_id ? "assigned" : (intervalAt(word) ?? "none")
    }`;
  const bySegment = new Map<string, SpeakerResolution>();
  const byWord = new Map<string, SpeakerResolution>();
  const byKey = new Map<string, SpeakerResolution>();
  for (const segment of previous) {
    const resolution = {
      speaker_label: segment.speaker_label,
      provisional_speaker: segment.provisional_speaker,
    };
    bySegment.set(segment.id, resolution);
    const first = segment.words[0];
    if (first) byKey.set(scopedKey(segment.key, first), resolution);
    for (const word of segment.words) {
      if (word.id) byWord.set(word.id, resolution);
    }
  }
  return segments.flatMap((segment) => {
    const whole = bySegment.get(segment.id);
    if (whole) return [withResolution(segment, whole)];

    const runs: Array<{
      scope: string;
      resolution?: SpeakerResolution;
      words: SegmentWord[];
    }> = [];
    let carried: SpeakerResolution | undefined;
    for (const word of segment.words) {
      const scope = scopedKey(segment.key, word);
      const run = runs[runs.length - 1];
      if (run?.scope !== scope) carried = byKey.get(scope);
      const resolution = (word.id ? byWord.get(word.id) : undefined) ?? carried;
      if (run && run.scope === scope && run.resolution === resolution) {
        run.words.push(word);
      } else {
        runs.push({ scope, resolution, words: [word] });
      }
      carried = resolution;
    }
    if (runs.length <= 1) {
      return [withResolution(segment, runs[0]?.resolution)];
    }
    return runs.map(({ resolution, words }) => {
      const start_ms = words[0]!.start_ms;
      return withResolution(
        {
          ...segment,
          id: `${segment.id}:${start_ms}`,
          start_ms,
          end_ms: words[words.length - 1]!.end_ms,
          text: words
            .map((word) => word.text)
            .join("")
            .trim(),
          words,
        },
        resolution,
      );
    });
  });
}

function createIntervalLookup(
  request: RenderTranscriptRequest,
): (word: SegmentWord) => number | undefined {
  const intervals = request.speaker_context?.intervals ?? [];
  const startedAt = request.transcripts.reduce<number | null>(
    (earliest, transcript) =>
      transcript.started_at === null
        ? earliest
        : Math.min(earliest ?? Infinity, transcript.started_at),
    null,
  );
  return (word) => {
    const start = (startedAt ?? 0) + word.start_ms;
    const end = (startedAt ?? 0) + word.end_ms;
    let found: number | undefined;
    for (const [index, interval] of intervals.entries()) {
      if (
        interval.start_ms <= start &&
        end <= interval.end_ms &&
        interval.start_ms < interval.end_ms
      ) {
        if (found !== undefined) return undefined;
        found = index;
      }
    }
    return found;
  };
}

function withResolution(
  segment: Segment,
  resolution: SpeakerResolution | undefined,
): Segment {
  return {
    ...segment,
    speaker_label: resolution?.speaker_label,
    provisional_speaker: resolution?.provisional_speaker,
  };
}
