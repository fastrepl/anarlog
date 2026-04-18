import { useCallback, useMemo } from "react";

import type { RenderTranscriptRequest } from "@hypr/plugin-transcription";

import {
  useCurrentUserId,
  useMainStoreInternal,
} from "~/session/hooks/internal";
import * as main from "~/store/tinybase/store/main";
import { buildRenderTranscriptRequestFromStore } from "~/stt/render-transcript";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";
import { parseTranscriptWords, upsertSpeakerAssignment } from "~/stt/utils";

export function useTranscriptIdsForSession(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );
}

export function useTranscriptTimeRange(sessionId: string): {
  startedAt: number | null;
  endedAt: number | null;
} {
  const transcriptIds = useTranscriptIdsForSession(sessionId);
  const transcripts = main.UI.useTable("transcripts", main.STORE_ID);

  return useMemo(() => {
    let minStartedAt: number | null = null;
    let maxEndedAt: number | null = null;

    for (const transcriptId of transcriptIds) {
      const transcript = transcripts[transcriptId];
      if (!transcript) continue;

      const startedAt = transcript.started_at;
      const endedAt = transcript.ended_at;

      if (typeof startedAt === "number") {
        minStartedAt =
          minStartedAt === null ? startedAt : Math.min(minStartedAt, startedAt);
      }

      if (typeof endedAt === "number") {
        maxEndedAt =
          maxEndedAt === null ? endedAt : Math.max(maxEndedAt, endedAt);
      }
    }

    return { startedAt: minStartedAt, endedAt: maxEndedAt };
  }, [transcriptIds, transcripts]);
}

export function useTranscriptHasWords(sessionId: string): boolean {
  const transcriptIds = useTranscriptIdsForSession(sessionId);
  const transcriptsTable = useTranscriptsTable();
  const store = useMainStoreInternal();

  return useMemo(() => {
    if (!store) return false;
    return transcriptIds.some(
      (transcriptId) => parseTranscriptWords(store, transcriptId).length > 0,
    );
  }, [store, transcriptIds, transcriptsTable]);
}

export function useTranscriptRenderRequest(
  transcriptIds: string[],
): RenderTranscriptRequest | null {
  const store = useMainStoreInternal();
  const transcriptsTable = useTranscriptsTable();
  const participantMappingsTable = useParticipantMappingsTable();
  const humansTable = useHumansTable();
  const selfHumanId = useCurrentUserId();

  return useMemo(() => {
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
}

export function useTranscriptOffsetMs(transcriptId: string): number {
  const transcriptStartedAt = main.UI.useCell(
    "transcripts",
    transcriptId,
    "started_at",
    main.STORE_ID,
  );
  const sessionId = useTranscriptSessionId(transcriptId);
  const transcriptIds = useTranscriptIdsForSession(sessionId);
  const transcriptsTable = useTranscriptsTable();

  return useMemo(() => {
    if (typeof transcriptStartedAt !== "number") {
      return 0;
    }

    let earliestStartedAt = Number.POSITIVE_INFINITY;
    for (const currentTranscriptId of transcriptIds ?? []) {
      const startedAt = transcriptsTable[currentTranscriptId]?.started_at;
      if (typeof startedAt === "number" && startedAt < earliestStartedAt) {
        earliestStartedAt = startedAt;
      }
    }

    return Number.isFinite(earliestStartedAt)
      ? transcriptStartedAt - earliestStartedAt
      : 0;
  }, [transcriptIds, transcriptStartedAt, transcriptsTable]);
}

export function useTranscriptLabelContext() {
  const store = useMainStoreInternal();
  return useMemo(
    () => (store ? defaultRenderLabelContext(store) : undefined),
    [store],
  );
}

export function useTranscriptSpeakerLabelManager(
  segments: Parameters<typeof SpeakerLabelManager.fromSegments>[0],
): SpeakerLabelManager {
  const labelContext = useTranscriptLabelContext();
  return useMemo(() => {
    if (!labelContext) {
      return new SpeakerLabelManager();
    }
    return SpeakerLabelManager.fromSegments(segments, labelContext);
  }, [labelContext, segments]);
}

export function useTranscriptsTable() {
  return main.UI.useTable("transcripts", main.STORE_ID);
}

export function useTranscriptSessionId(transcriptId: string): string {
  const v = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

export function useAssignTranscriptSpeaker(): (args: {
  transcriptId: string;
  segmentKey: unknown;
  humanId: string;
  anchorWordId: string;
}) => void {
  const store = useMainStoreInternal();

  return useCallback(
    ({ transcriptId, segmentKey, humanId, anchorWordId }) => {
      if (!store) return;

      upsertSpeakerAssignment(
        store,
        transcriptId,
        segmentKey as Parameters<typeof upsertSpeakerAssignment>[2],
        humanId,
        anchorWordId,
      );
    },
    [store],
  );
}

export function useParticipantMappingsTable() {
  return main.UI.useTable("mapping_session_participant", main.STORE_ID);
}

export function useHumansTable() {
  return main.UI.useTable("humans", main.STORE_ID);
}
