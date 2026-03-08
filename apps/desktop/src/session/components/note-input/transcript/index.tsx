import { type RefObject, useCallback } from "react";

import { TranscriptContainer } from "./shared";

import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import type { SpeakerHintWithId } from "~/stt/types";
import {
  parseTranscriptHints,
  parseTranscriptWords,
  updateTranscriptHints,
  updateTranscriptWords,
} from "~/stt/utils";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

function findTranscriptContainingWord(
  store: Store,
  indexes: ReturnType<typeof main.UI.useIndexes>,
  sessionId: string,
  wordId: string,
) {
  const transcriptIds = indexes?.getSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
  );
  if (!transcriptIds) return null;

  for (const transcriptId of transcriptIds) {
    const words = parseTranscriptWords(store, transcriptId);
    if (words.length === 0) continue;

    if (words.some((w) => w.id === wordId)) {
      const hints = parseTranscriptHints(store, transcriptId);
      return { transcriptId, words, hints };
    }
  }

  return null;
}

export function Transcript({
  sessionId,
  isEditing,
  scrollRef,
}: {
  sessionId: string;
  isEditing: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const checkpoints = main.UI.useCheckpoints(main.STORE_ID);

  const handleDeleteWord = useCallback(
    (wordId: string) => {
      if (!store || !indexes || !checkpoints) {
        return;
      }

      const found = findTranscriptContainingWord(
        store,
        indexes,
        sessionId,
        wordId,
      );
      if (!found) return;

      const { transcriptId, words, hints } = found;

      const updatedWords = words.filter((w) => w.id !== wordId);
      const updatedHints = hints.filter((h) => h.word_id !== wordId);

      updateTranscriptWords(store, transcriptId, updatedWords);
      updateTranscriptHints(store, transcriptId, updatedHints);

      checkpoints.addCheckpoint("delete_word");
    },
    [store, indexes, checkpoints, sessionId],
  );

  const handleAssignSpeaker = useCallback(
    (wordIds: string[], humanId: string) => {
      if (!store || !indexes || !checkpoints || wordIds.length === 0) {
        return;
      }

      const found = findTranscriptContainingWord(
        store,
        indexes,
        sessionId,
        wordIds[0],
      );
      if (!found) return;

      const { transcriptId, hints } = found;

      const newHints: SpeakerHintWithId[] = wordIds.map((wordId) => ({
        id: id(),
        user_id: "",
        created_at: new Date().toISOString(),
        transcript_id: transcriptId,
        word_id: wordId,
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: humanId }),
      }));

      updateTranscriptHints(store, transcriptId, [...hints, ...newHints]);

      checkpoints.addCheckpoint("assign_speaker");
    },
    [store, indexes, checkpoints, sessionId],
  );

  const operations = isEditing
    ? {
        onDeleteWord: handleDeleteWord,
        onAssignSpeaker: handleAssignSpeaker,
      }
    : undefined;

  return (
    <div className="relative flex h-full flex-col overflow-hidden">
      <TranscriptContainer
        sessionId={sessionId}
        operations={operations}
        scrollRef={scrollRef}
      />
    </div>
  );
}
