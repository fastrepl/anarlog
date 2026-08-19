import { md2json } from "@anlg/editor/markdown";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { catalogLocalSessionAudio, deleteSessionAudio } from "./attachments";
import { enqueueSessionAudioOperation } from "./audio-operations";
import { loadSessionContentSnapshot } from "./content-queries";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { listenerStore } from "~/store/zustand/listener/instance";

export type MoveSessionContentsResult =
  | {
      status: "moved";
      sourceMeetingId: string;
      targetMeetingId: string;
      sourceTitle: string;
      targetTitle: string;
      moved: {
        recording: boolean;
        transcripts: number;
        summaries: number;
        notes: boolean;
        actionItems: number;
      };
    }
  | {
      status: "error";
      message: string;
      sourceMeetingId?: string;
      targetMeetingId?: string;
    }
  | {
      status: "nothing_to_move";
      message: string;
      sourceMeetingId: string;
      targetMeetingId: string;
    };

type ActionItemCountRow = { action_item_count: number | boolean };

function hasNoteContent(markdown: string): boolean {
  const trimmed = markdown.trim();
  return Boolean(trimmed && trimmed !== "&nbsp;");
}

function emptyNoteBody(): string {
  return JSON.stringify(md2json(""));
}

function isSessionBusy(sessionId: string): boolean {
  const live = listenerStore.getState().live;
  if (
    live.sessionId === sessionId &&
    (live.status === "active" || live.status === "finalizing")
  ) {
    return true;
  }

  return Boolean(
    live.finalizingBySession[sessionId] ||
    live.batchTranscriptionPendingBySession[sessionId] ||
    live.postStopProcessingBySession[sessionId],
  );
}

function withOrderedLocks<T>(
  lock: (sessionId: string, operation: () => Promise<T>) => Promise<T>,
  sessionIds: string[],
  operation: () => Promise<T>,
): Promise<T> {
  const unique = [...new Set(sessionIds)].sort();
  let run = operation;
  for (let index = unique.length - 1; index >= 0; index--) {
    const sessionId = unique[index];
    const inner = run;
    run = () => lock(sessionId, inner);
  }
  return run();
}

async function sessionAudioExists(sessionId: string): Promise<boolean> {
  const result = await fsSyncCommands.audioExist(sessionId);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

async function copySessionAudio(
  sourceSessionId: string,
  targetSessionId: string,
): Promise<boolean> {
  const result = await fsSyncCommands.audioCopy(
    sourceSessionId,
    targetSessionId,
  );
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

async function rollbackCopiedAudio(targetSessionId: string): Promise<void> {
  try {
    await deleteSessionAudio(targetSessionId, () => true);
  } catch (error) {
    console.error(
      "[session] failed to roll back copied recording",
      targetSessionId,
      error,
    );
  }
}

export async function moveSessionContents({
  sourceSessionId,
  targetSessionId,
}: {
  sourceSessionId: string;
  targetSessionId: string;
}): Promise<MoveSessionContentsResult> {
  if (!sourceSessionId || !targetSessionId) {
    return {
      status: "error",
      message: "Both the source and target meetings are required.",
      sourceMeetingId: sourceSessionId || undefined,
      targetMeetingId: targetSessionId || undefined,
    };
  }

  if (sourceSessionId === targetSessionId) {
    return {
      status: "error",
      message: "Choose two different meetings.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }

  if (isSessionBusy(sourceSessionId) || isSessionBusy(targetSessionId)) {
    return {
      status: "error",
      message:
        "Wait until recording and transcription finish on both meetings, then try again.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }

  const [source, target] = await Promise.all([
    loadSessionContentSnapshot(sourceSessionId),
    loadSessionContentSnapshot(targetSessionId),
  ]);

  if (!source) {
    return {
      status: "error",
      message: "The source meeting could not be loaded.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }
  if (!target) {
    return {
      status: "error",
      message: "The target meeting could not be loaded.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }

  const [sourceHasAudio, targetHasAudio, actionItemRows] = await Promise.all([
    sessionAudioExists(sourceSessionId),
    sessionAudioExists(targetSessionId),
    liveQueryClient.execute<ActionItemCountRow>(
      `
        SELECT COUNT(*) AS action_item_count
        FROM action_items
        WHERE session_id = ? AND deleted_at IS NULL
      `,
      [sourceSessionId],
    ),
  ]);

  const sourceActionItems = Number(actionItemRows[0]?.action_item_count ?? 0);
  const sourceHasNotes = hasNoteContent(source.rawMarkdown);
  const hasAnythingToMove =
    sourceHasAudio ||
    source.transcripts.length > 0 ||
    source.enhancedNotes.length > 0 ||
    sourceHasNotes ||
    sourceActionItems > 0;

  if (!hasAnythingToMove) {
    return {
      status: "nothing_to_move",
      message:
        "The source meeting has no recording, transcript, or notes to move.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }

  if (targetHasAudio || target.transcripts.length > 0) {
    return {
      status: "error",
      message:
        "The target meeting already has a recording or transcript. Move into an empty meeting, or delete the existing recording first.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }

  let copiedAudio = false;
  try {
    if (sourceHasAudio) {
      copiedAudio = await withOrderedLocks(
        enqueueSessionAudioOperation,
        [sourceSessionId, targetSessionId],
        () => copySessionAudio(sourceSessionId, targetSessionId),
      );
      if (copiedAudio) {
        await catalogLocalSessionAudio(targetSessionId);
      }
    }

    const now = new Date().toISOString();
    const sourceAudioId = `session-audio:${sourceSessionId}`;
    const targetAudioId = `session-audio:${targetSessionId}`;
    const shouldRewriteAudioIds = copiedAudio;
    const targetHasNotes = hasNoteContent(target.rawMarkdown);
    const nextTargetNote = sourceHasNotes
      ? targetHasNotes
        ? JSON.stringify(
            md2json(
              [target.rawMarkdown.trim(), source.rawMarkdown.trim()].join(
                "\n\n",
              ),
            ),
          )
        : source.rawContentFormat === "prosemirror_json" && source.rawContent
          ? source.rawContent
          : JSON.stringify(md2json(source.rawMarkdown))
      : null;

    await withOrderedLocks(
      enqueueDatabaseWrite,
      [`session:${sourceSessionId}`, `session:${targetSessionId}`],
      () =>
        executeTransaction([
          {
            sql: `
              UPDATE transcripts
              SET
                session_id = ?,
                audio_attachment_id = CASE
                  WHEN ? = 1 AND audio_attachment_id = ? THEN ?
                  ELSE audio_attachment_id
                END,
                updated_at = ?
              WHERE session_id = ? AND deleted_at IS NULL
            `,
            params: [
              targetSessionId,
              shouldRewriteAudioIds ? 1 : 0,
              sourceAudioId,
              targetAudioId,
              now,
              sourceSessionId,
            ],
          },
          {
            sql: `
              UPDATE session_documents
              SET session_id = ?, updated_at = ?
              WHERE session_id = ?
                AND kind IN ('summary', 'template_output')
                AND deleted_at IS NULL
            `,
            params: [targetSessionId, now, sourceSessionId],
          },
          {
            sql: `
              UPDATE action_items
              SET session_id = ?, updated_at = ?
              WHERE session_id = ? AND deleted_at IS NULL
            `,
            params: [targetSessionId, now, sourceSessionId],
          },
          {
            sql: `
              UPDATE voiceprint_exemplars
              SET
                source_session_id = ?,
                source_attachment_id = CASE
                  WHEN ? = 1 AND source_attachment_id = ? THEN ?
                  ELSE source_attachment_id
                END,
                updated_at = ?
              WHERE source_session_id = ? AND deleted_at IS NULL
            `,
            params: [
              targetSessionId,
              shouldRewriteAudioIds ? 1 : 0,
              sourceAudioId,
              targetAudioId,
              now,
              sourceSessionId,
            ],
          },
          {
            sql: `
              UPDATE voiceprint_candidates
              SET
                source_session_id = ?,
                source_attachment_id = CASE
                  WHEN ? = 1 AND source_attachment_id = ? THEN ?
                  ELSE source_attachment_id
                END,
                updated_at = ?
              WHERE source_session_id = ? AND deleted_at IS NULL
            `,
            params: [
              targetSessionId,
              shouldRewriteAudioIds ? 1 : 0,
              sourceAudioId,
              targetAudioId,
              now,
              sourceSessionId,
            ],
          },
          ...(nextTargetNote
            ? [
                {
                  sql: `
                    UPDATE session_documents
                    SET body = ?, body_format = 'prosemirror_json', updated_at = ?
                    WHERE id = ?
                      AND session_id = ?
                      AND kind = 'note'
                      AND deleted_at IS NULL
                  `,
                  params: [
                    nextTargetNote,
                    now,
                    targetSessionId,
                    targetSessionId,
                  ],
                  expectedRowsAffected: 1,
                },
                {
                  sql: `
                    UPDATE session_documents
                    SET body = ?, body_format = 'prosemirror_json', updated_at = ?
                    WHERE id = ?
                      AND session_id = ?
                      AND kind = 'note'
                      AND deleted_at IS NULL
                  `,
                  params: [
                    emptyNoteBody(),
                    now,
                    sourceSessionId,
                    sourceSessionId,
                  ],
                  expectedRowsAffected: 1,
                },
              ]
            : []),
        ]),
    );
  } catch (error) {
    console.error("Failed to move meeting contents", error);
    if (copiedAudio) {
      await rollbackCopiedAudio(targetSessionId);
    }
    return {
      status: "error",
      message: "The move could not be completed. Nothing was changed.",
      sourceMeetingId: sourceSessionId,
      targetMeetingId: targetSessionId,
    };
  }

  if (copiedAudio) {
    try {
      await deleteSessionAudio(sourceSessionId, () => true);
    } catch (error) {
      console.error(
        "[session] moved recording but failed to remove the source file",
        error,
      );
    }
  }

  return {
    status: "moved",
    sourceMeetingId: sourceSessionId,
    targetMeetingId: targetSessionId,
    sourceTitle: source.title,
    targetTitle: target.title,
    moved: {
      recording: copiedAudio,
      transcripts: source.transcripts.length,
      summaries: source.enhancedNotes.length,
      notes: sourceHasNotes,
      actionItems: sourceActionItems,
    },
  };
}
