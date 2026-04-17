import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import {
  useCurrentUserId,
  useHumansTable,
  useMainStore,
  useParticipantMappingsTable,
  useTranscriptIdsForSession,
  useTranscriptSessionId,
  useTranscriptsTable,
} from "~/session/hooks/storage";
import type { Segment } from "~/stt/live-segment";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

export function useRenderedTranscriptSegments(transcriptId: string): Segment[] {
  const store = useMainStore();
  const transcriptsTable = useTranscriptsTable();
  const participantMappingsTable = useParticipantMappingsTable();
  const humansTable = useHumansTable();
  const selfHumanId = useCurrentUserId();

  const request = useMemo(() => {
    if (!store) {
      return null;
    }

    return buildRenderTranscriptRequestFromStore(store, [transcriptId]);
  }, [
    store,
    transcriptId,
    transcriptsTable,
    participantMappingsTable,
    humansTable,
    selfHumanId,
  ]);

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
  const store = useMainStore();
  const transcriptsTable = useTranscriptsTable();
  const sessionId = useTranscriptSessionId(transcriptId);
  const transcriptIds = useTranscriptIdsForSession(sessionId);

  return useMemo(() => {
    if (!store) {
      return 0;
    }

    const transcriptStartedAt = store.getCell(
      "transcripts",
      transcriptId,
      "started_at",
    );
    if (typeof transcriptStartedAt !== "number") {
      return 0;
    }

    let earliestStartedAt = Number.POSITIVE_INFINITY;
    for (const currentTranscriptId of transcriptIds ?? []) {
      const startedAt = store.getCell(
        "transcripts",
        currentTranscriptId,
        "started_at",
      );
      if (typeof startedAt === "number" && startedAt < earliestStartedAt) {
        earliestStartedAt = startedAt;
      }
    }

    return Number.isFinite(earliestStartedAt)
      ? transcriptStartedAt - earliestStartedAt
      : 0;
  }, [store, transcriptId, transcriptIds, transcriptsTable]);
}
