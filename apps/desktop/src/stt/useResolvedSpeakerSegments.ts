import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import {
  commands,
  type RenderTranscriptRequest,
  type RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

import {
  type Segment,
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
    if (!data) return undefined;
    if (isPlaceholderData) return carrySpeakerResolution(segments, data);
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
  }, [data, isPlaceholderData, segments]);
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
): Segment[] {
  const bySegment = new Map<string, SpeakerResolution>();
  const byWord = new Map<string, SpeakerResolution>();
  const byKey = new Map<string, SpeakerResolution>();
  for (const segment of previous) {
    const resolution = {
      speaker_label: segment.speaker_label,
      provisional_speaker: segment.provisional_speaker,
    };
    bySegment.set(segment.id, resolution);
    byKey.set(SegmentKeyUtils.serialize(segment.key), resolution);
    for (const word of segment.words) {
      if (word.id) byWord.set(word.id, resolution);
    }
  }
  return segments.flatMap((segment) => {
    const whole = bySegment.get(segment.id);
    if (whole) return [withResolution(segment, whole)];

    // The native labeler splits a segment whose words straddle a speaker
    // context boundary, so carry each run separately and mirror its ids.
    const runs: Array<{
      resolution?: SpeakerResolution;
      words: SegmentWord[];
    }> = [];
    let carried = byKey.get(SegmentKeyUtils.serialize(segment.key));
    for (const word of segment.words) {
      const resolution = (word.id ? byWord.get(word.id) : undefined) ?? carried;
      const run = runs[runs.length - 1];
      if (run && run.resolution === resolution) {
        run.words.push(word);
      } else {
        runs.push({ resolution, words: [word] });
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
