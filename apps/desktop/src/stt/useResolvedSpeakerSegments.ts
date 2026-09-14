import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import {
  commands,
  type RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import type { Segment } from "~/stt/live-segment";

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
  const { data } = useQuery({
    queryKey: ["contextual-speaker-segments", input],
    queryFn: async () => {
      if (!input) return [];
      const result = await commands.renderTranscriptSegments(input);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    enabled: Boolean(input),
    staleTime: Infinity,
    gcTime: 0,
  });
  const resolved = useMemo(() => {
    if (!data) return undefined;
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
  }, [data, segments]);
  return input
    ? (resolved ??
        segments.map((segment) => ({
          ...segment,
          speaker_label: undefined,
          provisional_speaker: undefined,
        })))
    : segments;
}
