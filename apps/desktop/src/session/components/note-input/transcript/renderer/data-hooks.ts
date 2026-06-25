import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { TRANSCRIPT_RENDER_CACHE_TIME_MS } from "../cache";
import {
  useTranscriptRenderData,
  useTranscriptRowsRevision,
} from "../render-request-hooks";

import * as main from "~/store/tinybase/store/main";
import {
  getMaxSpeakerNumberForParticipants,
  type Segment,
} from "~/stt/live-segment";
import {
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

const emptyIds: string[] = [];

export function useRenderedTranscriptSegments(transcriptId: string): Segment[] {
  return useRenderedTranscriptData(transcriptId).segments;
}

export function useRenderedTranscriptData(transcriptId: string): {
  maxSpeakerNumber?: number;
  segments: Segment[];
} {
  const { request } = useTranscriptRenderData(transcriptId);
  const requestKey = useMemo(
    () => getRenderTranscriptRequestKey(request),
    [request],
  );

  const { data = [] } = useQuery({
    queryKey: ["rendered-transcript-segments", transcriptId, requestKey],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: TRANSCRIPT_RENDER_CACHE_TIME_MS,
  });

  const maxSpeakerNumber = useMemo(
    () =>
      request
        ? getMaxSpeakerNumberForParticipants(
            request.participant_human_ids,
            request.self_human_id,
          )
        : undefined,
    [request],
  );

  return { maxSpeakerNumber, segments: data };
}

export function useTranscriptOffset(transcriptId: string): number {
  const store = main.UI.useStore(main.STORE_ID);
  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  );

  const transcriptIds =
    main.UI.useSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId ?? "",
      main.STORE_ID,
    ) ?? emptyIds;
  const transcriptRowsRevision = useTranscriptRowsRevision(transcriptIds);

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
  }, [store, transcriptId, transcriptIds, transcriptRowsRevision]);
}
