import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import type { TranscriptItem } from "@anlg/plugin-export";
import type { RenderTranscriptRequest } from "@anlg/plugin-transcription";

import { TRANSCRIPT_RENDER_CACHE_TIME_MS } from "./cache";
import { useSessionTranscriptRenderData } from "./render-request-hooks";

import {
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
} from "~/stt/render-transcript";
import { applyStoredTranscriptEdits } from "~/stt/transcript-edits";

export type TranscriptExportSegment = TranscriptItem & {
  start_ms: number;
  end_ms: number;
};

export async function buildTranscriptExportSegments(
  request: RenderTranscriptRequest,
  transcriptRows: Array<{ transcriptId: string; metadataJson: string }> = [],
): Promise<TranscriptExportSegment[]> {
  const rendered = await renderTranscriptSegments(request);
  const segments = applyStoredTranscriptEdits(rendered, transcriptRows);

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
  const { request, transcriptRows } = useSessionTranscriptRenderData(sessionId);
  const requestKey = useMemo(
    () => getRenderTranscriptRequestKey(request),
    [request],
  );

  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- the query key includes the request hash and transcript edit metadata.
  const { data = [], isLoading } = useQuery({
    queryKey: [
      "transcript-export-segments",
      sessionId,
      requestKey,
      transcriptRows.map(({ transcriptId, metadataJson }) => [
        transcriptId,
        metadataJson,
      ]),
    ],
    queryFn: async () => {
      if (!request) {
        return [];
      }
      return buildTranscriptExportSegments(request, transcriptRows);
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
