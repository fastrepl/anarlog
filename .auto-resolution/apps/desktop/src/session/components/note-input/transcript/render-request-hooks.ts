import { useMemo } from "react";

import type { RenderTranscriptRequest } from "@hypr/plugin-transcription";

import {
  type TranscriptRecord,
  useSessionParticipantHumanIds,
  useSessionTranscripts,
  useTranscript,
  useTranscriptHumans,
} from "~/stt/queries";
import {
  buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows,
  type TranscriptRow,
} from "~/stt/render-transcript";

export type TranscriptRowWithId = {
  transcriptId: string;
  row: TranscriptRow;
};

export function useTranscriptRenderData(transcriptId: string): {
  request: RenderTranscriptRequest | null;
  transcriptRows: TranscriptRowWithId[];
} {
  const transcript = useTranscript(transcriptId);
  const transcripts = useMemo(
    () => (transcript ? [transcript] : emptyTranscripts),
    [transcript],
  );

  return useRenderData(transcript?.sessionId ?? "", transcripts);
}

export function useSessionTranscriptRenderData(sessionId: string): {
  request: RenderTranscriptRequest | null;
  transcriptRows: TranscriptRowWithId[];
} {
  const transcripts = useSessionTranscripts(sessionId);

  return useRenderData(sessionId, transcripts);
}

function useRenderData(
  sessionId: string,
  transcripts: readonly TranscriptRecord[],
): {
  request: RenderTranscriptRequest | null;
  transcriptRows: TranscriptRowWithId[];
} {
  const participantHumanIds = useSessionParticipantHumanIds(sessionId);
  const selfHumanId = transcripts[0]?.ownerUserId;

  const transcriptRows = useMemo(() => {
    return transcripts.map((transcript) => ({
      transcriptId: transcript.id,
      row: {
        started_at: transcript.startedAt,
        words: transcript.words,
        speaker_hints: transcript.speakerHints,
      },
    }));
  }, [transcripts]);

  const assignedHumanIds = useMemo(
    () =>
      collectAssignedHumanIdsFromTranscriptRows(
        transcriptRows.map((transcriptRow) => transcriptRow.row),
      ),
    [transcriptRows],
  );

  const humanIds = useMemo(
    () =>
      [
        ...new Set([
          ...participantHumanIds,
          ...assignedHumanIds,
          selfHumanId ?? "",
        ]),
      ].filter(Boolean),
    [assignedHumanIds, participantHumanIds, selfHumanId],
  );
  const humans = useTranscriptHumans(humanIds);

  const request = useMemo(
    () =>
      buildRenderTranscriptRequestFromRows(
        transcriptRows.map((transcriptRow) => transcriptRow.row),
        { humans, selfHumanId },
        participantHumanIds,
      ),
    [humans, participantHumanIds, selfHumanId, transcriptRows],
  );

  return { request, transcriptRows };
}

const emptyTranscripts: TranscriptRecord[] = [];
