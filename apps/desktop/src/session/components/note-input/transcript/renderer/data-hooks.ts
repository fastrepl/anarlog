import { useMemo } from "react";

import type { SpeakerHintStorage, Word } from "@hypr/store";
import type { RuntimeSpeakerHint } from "@hypr/transcript";

import * as main from "~/store/tinybase/store/main";
import { convertStorageHintsToRuntime } from "~/stt/speaker-hints";

export function useTranscriptData(transcriptId: string): {
  words: (Word & { id: string })[];
  speakerHints: RuntimeSpeakerHint[];
} {
  const wordsJson = main.UI.useCell(
    "transcripts",
    transcriptId,
    "words",
    main.STORE_ID,
  ) as string | undefined;

  const speakerHintsJson = main.UI.useCell(
    "transcripts",
    transcriptId,
    "speaker_hints",
    main.STORE_ID,
  ) as string | undefined;

  return useMemo(() => {
    if (!wordsJson) {
      return { words: [], speakerHints: [] };
    }

    let words: (Word & { id: string })[];
    try {
      words = JSON.parse(wordsJson) as (Word & { id: string })[];
    } catch {
      return { words: [], speakerHints: [] };
    }

    if (!speakerHintsJson) {
      return { words, speakerHints: [] };
    }

    let storageHints: Array<SpeakerHintStorage & { id: string }>;
    try {
      storageHints = JSON.parse(speakerHintsJson);
    } catch {
      return { words, speakerHints: [] };
    }

    const wordIdToIndex = new Map<string, number>();
    words.forEach((word, index) => {
      wordIdToIndex.set(word.id, index);
    });

    const speakerHints = convertStorageHintsToRuntime(
      storageHints,
      wordIdToIndex,
    );
    return { words, speakerHints };
  }, [wordsJson, speakerHintsJson]);
}

export function useTranscriptOffset(transcriptId: string): number {
  const transcriptStartedAt = main.UI.useCell(
    "transcripts",
    transcriptId,
    "started_at",
    main.STORE_ID,
  );

  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  );

  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId ?? "",
    main.STORE_ID,
  );

  const firstTranscriptId = transcriptIds?.[0];
  const firstTranscriptStartedAt = main.UI.useCell(
    "transcripts",
    firstTranscriptId ?? "",
    "started_at",
    main.STORE_ID,
  );

  return transcriptStartedAt && firstTranscriptStartedAt
    ? new Date(transcriptStartedAt).getTime() -
        new Date(firstTranscriptStartedAt).getTime()
    : 0;
}

export function useSessionSpeakerCount(sessionId?: string) {
  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId ?? "",
    main.STORE_ID,
  ) as string[];

  if (!sessionId) {
    return undefined;
  }

  return mappingIds.length;
}
