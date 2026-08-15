import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useMemo, useRef } from "react";

import type { RenderTranscriptRequest } from "@anlg/plugin-transcription";

import { TRANSCRIPT_RENDER_CACHE_TIME_MS } from "../cache";
import { useTranscriptRenderData } from "../render-request-hooks";

import {
  getMaxSpeakerNumberForParticipants,
  type Segment,
} from "~/stt/live-segment";
import {
  useSessionTranscriptMetadata,
  useTranscriptMetadata,
} from "~/stt/queries";
import {
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

export function useRenderedTranscriptSegments(transcriptId: string): Segment[] {
  return useRenderedTranscriptData(transcriptId).segments;
}

export function useRenderedTranscriptData(
  transcriptId: string,
  currentActive = false,
  captureGeneration = 0,
): {
  maxSpeakerNumber?: number;
  request: RenderTranscriptRequest | null;
  segments: Segment[];
} {
  const { request } = useTranscriptRenderData(transcriptId, !currentActive);
  // Recovery needs the persisted prefix. The active key stays stable across
  // word and assignment writes so tab remounts reuse the same native render.
  const activeBaselineRef = useRef<{
    captureGeneration: number;
    transcriptId: string;
    request: typeof request;
  } | null>(null);
  if (!currentActive) {
    activeBaselineRef.current = null;
  } else if (
    activeBaselineRef.current?.transcriptId !== transcriptId ||
    activeBaselineRef.current.captureGeneration !== captureGeneration ||
    activeBaselineRef.current.request === null
  ) {
    activeBaselineRef.current = {
      captureGeneration,
      transcriptId,
      request,
    };
  }
  const activeBaselineRequest = currentActive
    ? (activeBaselineRef.current?.request ?? null)
    : request;
  const requestKey = useMemo(
    () =>
      currentActive
        ? `baseline:${captureGeneration}`
        : getRenderTranscriptRequestKey(request),
    [captureGeneration, currentActive, request],
  );

  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- active input is frozen and reconciled with current SQLite state in JavaScript.
  const { data = [] } = useQuery({
    queryKey: [
      "rendered-transcript-segments",
      transcriptId,
      currentActive ? "volatile" : "settled",
      requestKey,
    ],
    queryFn: async () => {
      if (!activeBaselineRequest) {
        return [];
      }

      return renderTranscriptSegments(activeBaselineRequest);
    },
    enabled: !!activeBaselineRequest,
    placeholderData: keepPreviousData,
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

  return { maxSpeakerNumber, request, segments: data };
}

export function useTranscriptTimelineMetadata(transcriptId: string): {
  offsetMs: number;
  sessionId?: string;
} {
  const transcript = useTranscriptMetadata(transcriptId);
  const transcripts = useSessionTranscriptMetadata(transcript?.sessionId ?? "");

  return useMemo(() => {
    if (!transcript) {
      return { offsetMs: 0 };
    }

    const earliestStartedAt = Math.min(
      ...transcripts.map((current) => current.startedAt),
    );

    return {
      offsetMs: Number.isFinite(earliestStartedAt)
        ? transcript.startedAt - earliestStartedAt
        : 0,
      sessionId: transcript.sessionId,
    };
  }, [transcript, transcripts]);
}
