import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import type { TranscriptItem } from "@hypr/plugin-export";
import type { RenderTranscriptRequest } from "@hypr/plugin-transcription";

import { TRANSCRIPT_RENDER_CACHE_TIME_MS } from "./cache";
import { useSessionTranscriptRenderData } from "./render-request-hooks";

import {
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

export type TranscriptExportSegment = TranscriptItem & {
  start_ms: number;
  end_ms: number;
};

export async function buildTranscriptExportSegments(
  request: RenderTranscriptRequest,
): Promise<TranscriptExportSegment[]> {
  const segments = await renderTranscriptSegments(request);

  return segments.map((segment) => ({
    text: segment.text,
    start_ms: segment.start_ms,
    end_ms: segment.end_ms,
    speaker: segment.speaker_label,
  }));
}

export function useTranscriptExportSegments(sessionId: string): {
  data: TranscriptExportSegment[];
  isLoading: boolean;
} {
  const { request } = useSessionTranscriptRenderData(sessionId);
  const requestKey = useMemo(
    () => getRenderTranscriptRequestKey(request),
    [request],
  );

  const { data = [], isLoading } = useQuery({
    queryKey: ["transcript-export-segments", sessionId, requestKey],
    queryFn: async () => {
      if (!request) {
        return [];
      }
      return buildTranscriptExportSegments(request);
    },
    enabled: !!request,
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: TRANSCRIPT_RENDER_CACHE_TIME_MS,
  });

  return { data, isLoading };
}

export function formatTranscriptExportSegments(
  segments: Array<{ speaker: string | null; text: string }>,
) {
  return segments
    .map((segment) => `${segment.speaker ?? "Speaker"}: ${segment.text}`)
    .join("\n\n");
}
