import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { TranscriptStorage } from "@hypr/store";

import * as main from "./main";

import type { DeletedSessionData } from "~/store/zustand/undo-delete";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
type Indexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;

function deleteByIndex(
  store: Store,
  indexes: Indexes,
  indexName: string,
  key: string,
  tableName: (typeof main.TABLES)[number],
): void {
  const ids = indexes.getSliceRowIds(indexName, key);
  for (const id of ids) {
    store.delRow(tableName, id);
  }
}

export function captureSessionData(
  store: Store,
  indexes: Indexes | undefined,
  sessionId: string,
): DeletedSessionData | null {
  const sessionRow = store.getRow("sessions", sessionId);
  if (!sessionRow || Object.keys(sessionRow).length === 0) {
    return null;
  }

  const session = {
    id: sessionId,
    user_id: sessionRow.user_id as string,
    created_at: sessionRow.created_at as string,
    folder_id: sessionRow.folder_id as string,
    event_json: sessionRow.event_json as string,
    title: sessionRow.title as string,
    raw_md: sessionRow.raw_md as string,
  };

  const transcripts: DeletedSessionData["transcripts"] = [];
  const participants: DeletedSessionData["participants"] = [];
  const tagSessions: DeletedSessionData["tagSessions"] = [];
  const enhancedNotes: DeletedSessionData["enhancedNotes"] = [];

  if (indexes) {
    const transcriptIds = indexes.getSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId,
    );
    for (const id of transcriptIds) {
      const row = store.getRow("transcripts", id);
      if (row && Object.keys(row).length > 0) {
        transcripts.push({
          id,
          user_id: row.user_id as string,
          created_at: row.created_at as string,
          session_id: row.session_id as string,
          started_at: row.started_at as number,
          ended_at: row.ended_at as number,
          words: row.words as string,
          speaker_hints: row.speaker_hints as string,
          memo_md: (row.memo_md as string) ?? "",
        });
      }
    }

    const participantIds = indexes.getSliceRowIds(
      main.INDEXES.sessionParticipantsBySession,
      sessionId,
    );
    for (const id of participantIds) {
      const row = store.getRow("mapping_session_participant", id);
      if (row && Object.keys(row).length > 0) {
        participants.push({
          id,
          user_id: row.user_id as string,
          session_id: row.session_id as string,
          human_id: row.human_id as string,
          source: row.source as string,
        });
      }
    }

    const tagSessionIds = indexes.getSliceRowIds(
      main.INDEXES.tagSessionsBySession,
      sessionId,
    );
    for (const id of tagSessionIds) {
      const row = store.getRow("mapping_tag_session", id);
      if (row && Object.keys(row).length > 0) {
        tagSessions.push({
          id,
          user_id: row.user_id as string,
          tag_id: row.tag_id as string,
          session_id: row.session_id as string,
        });
      }
    }

    const enhancedNoteIds = indexes.getSliceRowIds(
      main.INDEXES.enhancedNotesBySession,
      sessionId,
    );
    for (const id of enhancedNoteIds) {
      const row = store.getRow("enhanced_notes", id);
      if (row && Object.keys(row).length > 0) {
        enhancedNotes.push({
          id,
          user_id: row.user_id as string,
          session_id: row.session_id as string,
          content: row.content as string,
          template_id: row.template_id as string,
          position: row.position as number,
          title: row.title as string,
        });
      }
    }
  }

  return {
    session,
    transcripts,
    participants,
    tagSessions,
    enhancedNotes,
    deletedAt: Date.now(),
  };
}

export function restoreSessionData(
  store: Store,
  data: DeletedSessionData,
): void {
  store.transaction(() => {
    const { session, transcripts, participants, tagSessions, enhancedNotes } =
      data;

    store.setRow("sessions", session.id, {
      user_id: session.user_id,
      created_at: session.created_at,
      folder_id: session.folder_id,
      event_json: session.event_json,
      title: session.title,
      raw_md: session.raw_md,
    });

    for (const transcript of transcripts) {
      const transcriptRow = {
        user_id: transcript.user_id,
        created_at: transcript.created_at,
        session_id: transcript.session_id,
        started_at: transcript.started_at,
        ended_at: transcript.ended_at,
        words: transcript.words,
        speaker_hints: transcript.speaker_hints,
        memo_md: transcript.memo_md,
      } satisfies TranscriptStorage;

      store.setRow("transcripts", transcript.id, transcriptRow);
    }

    for (const participant of participants) {
      store.setRow("mapping_session_participant", participant.id, {
        user_id: participant.user_id,
        session_id: participant.session_id,
        human_id: participant.human_id,
        source: participant.source,
      });
    }

    for (const tagSession of tagSessions) {
      store.setRow("mapping_tag_session", tagSession.id, {
        user_id: tagSession.user_id,
        tag_id: tagSession.tag_id,
        session_id: tagSession.session_id,
      });
    }

    for (const enhancedNote of enhancedNotes) {
      store.setRow("enhanced_notes", enhancedNote.id, {
        user_id: enhancedNote.user_id,
        session_id: enhancedNote.session_id,
        content: enhancedNote.content,
        template_id: enhancedNote.template_id,
        position: enhancedNote.position,
        title: enhancedNote.title,
      });
    }
  });
}

export function deleteSessionCascade(
  store: Store,
  indexes: ReturnType<typeof main.UI.useIndexes>,
  sessionId: string,
  options?: { skipAudio?: boolean },
): void {
  if (!indexes) {
    store.delRow("sessions", sessionId);
  } else {
    store.transaction(() => {
      const transcriptIds = indexes.getSliceRowIds(
        main.INDEXES.transcriptBySession,
        sessionId,
      );

      for (const transcriptId of transcriptIds) {
        store.delRow("transcripts", transcriptId);
      }

      deleteByIndex(
        store,
        indexes,
        main.INDEXES.sessionParticipantsBySession,
        sessionId,
        "mapping_session_participant",
      );
      deleteByIndex(
        store,
        indexes,
        main.INDEXES.tagSessionsBySession,
        sessionId,
        "mapping_tag_session",
      );
      deleteByIndex(
        store,
        indexes,
        main.INDEXES.enhancedNotesBySession,
        sessionId,
        "enhanced_notes",
      );

      store.delRow("sessions", sessionId);
    });
  }

  if (!options?.skipAudio) {
    void fsSyncCommands.audioDelete(sessionId);
  }
}
