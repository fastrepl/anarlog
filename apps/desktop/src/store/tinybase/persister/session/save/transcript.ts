import { sep } from "@tauri-apps/api/path";

import type { TranscriptJson, TranscriptWithData } from "@hypr/plugin-fs-sync";

import {
  buildSessionPath,
  iterateTableRows,
  SESSION_TRANSCRIPT_FILE,
  type TablesContent,
  type WriteOperation,
} from "~/store/tinybase/persister/shared";
import { MIN_WORDS_FOR_MEANINGFUL_TRANSCRIPT } from "~/stt/thresholds";

type BuildContext = {
  tables: TablesContent;
  dataDir: string;
  changedSessionIds?: Set<string>;
};

export function buildTranscriptSaveOps(
  tables: TablesContent,
  dataDir: string,
  changedSessionIds?: Set<string>,
): WriteOperation[] {
  const ctx: BuildContext = { tables, dataDir, changedSessionIds };

  const transcriptsBySession = groupTranscriptsBySession(ctx);
  const sessionIdsToProcess = getSessionIdsToProcess(
    tables,
    transcriptsBySession,
    changedSessionIds,
  );

  return buildOperations(ctx, transcriptsBySession, sessionIdsToProcess);
}

function groupTranscriptsBySession(
  ctx: BuildContext,
): Map<string, TranscriptWithData[]> {
  const { tables } = ctx;
  const grouped = new Map<string, TranscriptWithData[]>();

  for (const transcript of iterateTableRows(tables, "transcripts")) {
    if (!transcript.session_id) continue;

    const data: TranscriptWithData = {
      id: transcript.id,
      user_id: transcript.user_id ?? "",
      created_at: transcript.created_at ?? "",
      session_id: transcript.session_id,
      started_at: transcript.started_at ?? 0,
      memo_md: transcript.memo_md ?? "",
      ended_at: transcript.ended_at,
      words: transcript.words ? JSON.parse(transcript.words) : [],
      speaker_hints: transcript.speaker_hints
        ? JSON.parse(transcript.speaker_hints)
        : [],
    };

    const list = grouped.get(transcript.session_id) ?? [];
    list.push(data);
    grouped.set(transcript.session_id, list);
  }

  return grouped;
}

function getSessionIdsToProcess(
  tables: TablesContent,
  transcriptsBySession: Map<string, TranscriptWithData[]>,
  changedSessionIds?: Set<string>,
): string[] {
  if (changedSessionIds) {
    return [...changedSessionIds];
  }

  return [
    ...new Set([
      ...Object.keys(tables.sessions ?? {}),
      ...transcriptsBySession.keys(),
    ]),
  ];
}

function buildOperations(
  ctx: BuildContext,
  transcriptsBySession: Map<string, TranscriptWithData[]>,
  sessionIds: string[],
): WriteOperation[] {
  const { tables, dataDir } = ctx;
  const operations: WriteOperation[] = [];
  const deletePaths: string[] = [];

  sessionIds.forEach((sessionId) => {
    const transcripts = transcriptsBySession.get(sessionId) ?? [];
    const session = tables.sessions?.[sessionId];
    const sessionDir = buildSessionPath(
      dataDir,
      sessionId,
      session?.folder_id ?? "",
    );
    const path = [sessionDir, SESSION_TRANSCRIPT_FILE].join(sep());
    const wordCount = transcripts.reduce(
      (total, transcript) => total + (transcript.words?.length ?? 0),
      0,
    );

    if (wordCount < MIN_WORDS_FOR_MEANINGFUL_TRANSCRIPT) {
      deletePaths.push(path);
      return;
    }

    const content: TranscriptJson = { transcripts };
    operations.push({
      type: "write-json" as const,
      path,
      content,
    });
  });

  if (deletePaths.length > 0) {
    operations.push({
      type: "delete",
      paths: deletePaths,
    });
  }

  return operations;
}
