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

// The native labeler request for live segments: the transcript rows only anchor
// the capture start, the segments themselves travel as the preview.
export function buildSpeakerResolutionInput(
  segments: Segment[],
  request: RenderTranscriptRequest | null,
): RenderTranscriptRequest | null {
  return request?.speaker_context && segments.length
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
    : null;
}

export function useResolvedSpeakerSegments(
  segments: Segment[],
  request: RenderTranscriptRequest | null,
): Segment[] {
  const input = useMemo(
    () => buildSpeakerResolutionInput(segments, request),
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
  // Within one native result a name depends only on the speaker key and the
  // speaker-context interval, and a segment whose words straddle a boundary is
  // split, so carry the previous name per scope and emit the labeler's ids.
  const intervalAt = createIntervalLookup(request);
  const scopeOf = (key: SegmentKey, word: SegmentWord) =>
    `${SegmentKeyUtils.serialize(key)}:${
      key.speaker_human_id ? "assigned" : (intervalAt(word) ?? "none")
    }`;
  const byScope = new Map<string, SpeakerResolution>();
  for (const segment of previous) {
    const first = segment.words[0];
    if (!first) continue;
    byScope.set(scopeOf(segment.key, first), {
      speaker_label: segment.speaker_label,
      provisional_speaker: segment.provisional_speaker,
    });
  }
  return segments.flatMap((segment) => {
    const runs: Array<{ scope: string; words: SegmentWord[] }> = [];
    for (const word of segment.words) {
      const scope = scopeOf(segment.key, word);
      const run = runs[runs.length - 1];
      if (run?.scope === scope) {
        run.words.push(word);
      } else {
        runs.push({ scope, words: [word] });
      }
    }
    if (runs.length <= 1) {
      return [withResolution(segment, runs[0] && byScope.get(runs[0].scope))];
    }
    return runs.map(({ scope, words }) => {
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
        byScope.get(scope),
      );
    });
  });
}

function createIntervalLookup(
  request: RenderTranscriptRequest,
): (word: SegmentWord) => number | undefined {
  const intervals = request.speaker_context?.intervals ?? [];
  const earliest = Math.min(
    ...request.transcripts.flatMap((transcript) =>
      transcript.started_at === null ? [] : [transcript.started_at],
    ),
  );
  const startedAt = Number.isFinite(earliest) ? earliest : 0;
  return (word) => {
    const start = startedAt + word.start_ms;
    const end = startedAt + word.end_ms;
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
