import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import type { TranscriptItem } from "@hypr/plugin-export";

import {
  useCurrentUserId,
  useHumansTable,
  useMainStore,
  useParticipantMappingsTable,
  useTranscriptIdsForSession,
  useTranscriptsTable,
} from "~/session/hooks/storage";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

export type TranscriptExportSegment = TranscriptItem & {
  start_ms: number;
  end_ms: number;
};

export async function buildTranscriptExportSegments(
  request: NonNullable<
    ReturnType<typeof buildRenderTranscriptRequestFromStore>
  >,
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
  const store = useMainStore();
  const transcriptsTable = useTranscriptsTable();
  const participantMappingsTable = useParticipantMappingsTable();
  const humansTable = useHumansTable();
  const selfHumanId = useCurrentUserId();

  const transcriptIds = useTranscriptIdsForSession(sessionId) ?? [];

  const request = useMemo(() => {
    if (!store || transcriptIds.length === 0) {
      return null;
    }

    return buildRenderTranscriptRequestFromStore(store, transcriptIds);
  }, [
    store,
    transcriptIds,
    transcriptsTable,
    participantMappingsTable,
    humansTable,
    selfHumanId,
  ]);

  const { data = [], isLoading } = useQuery({
    queryKey: ["transcript-export-segments", sessionId, request],
    queryFn: async () => {
      if (!request) {
        return [];
      }
      return buildTranscriptExportSegments(request);
    },
    enabled: !!request,
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
