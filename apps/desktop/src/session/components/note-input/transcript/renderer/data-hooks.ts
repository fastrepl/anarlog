import { useQuery } from "@tanstack/react-query";

import {
  useTranscriptOffsetMs,
  useTranscriptRenderRequest,
} from "~/session/hooks/storage";
import type { Segment } from "~/stt/live-segment";
import { renderTranscriptSegments } from "~/stt/render-transcript";

export function useRenderedTranscriptSegments(transcriptId: string): Segment[] {
  const request = useTranscriptRenderRequest([transcriptId]);

  const { data = [] } = useQuery({
    queryKey: ["rendered-transcript-segments", transcriptId, request],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
  });

  return data;
}

export function useTranscriptOffset(transcriptId: string): number {
  return useTranscriptOffsetMs(transcriptId);
}
