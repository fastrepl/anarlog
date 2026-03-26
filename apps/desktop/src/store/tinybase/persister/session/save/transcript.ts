import { sep } from "@tauri-apps/api/path";

import type { TranscriptJson, TranscriptWithData } from "@hypr/plugin-fs-sync";

import {
  buildSessionPath,
  iterateTableRows,
  SESSION_TRANSCRIPT_FILE,
  type TablesContent,
  type WriteOperation,
} from "~/store/tinybase/persister/shared";

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
  return buildOperations(ctx, transcriptsBySession);
}

function groupTranscriptsBySession(
  ctx: BuildContext,
): Map<string, TranscriptWithData[]> {
  const { tables } = ctx;
  const grouped = new Map<string, TranscriptWithData[]>();

  for (const transcript of iterateTableRows(tables, "transcripts")) {
    if (!transcript.session_id) continue;
    const words = transcript.words ? JSON.parse(transcript.words) : [];

    const data: TranscriptWithData = {
      id: transcript.id,
      user_id: transcript.user_id ?? "",
      created_at: transcript.created_at ?? "",
      session_id: transcript.session_id,
      started_at: transcript.started_at ?? 0,
      memo_md: transcript.memo_md ?? "",
      ended_at: transcript.ended_at,
      words,
      speaker_hints: transcript.speaker_hints
        ? JSON.parse(transcript.speaker_hints)
        : [],
    };
    if (words.length === 0) continue;

    const list = grouped.get(transcript.session_id) ?? [];
    list.push(data);
    grouped.set(transcript.session_id, list);
  }

  return grouped;
}

function buildOperations(
  ctx: BuildContext,
  transcriptsBySession: Map<string, TranscriptWithData[]>,
): WriteOperation[] {
  const { tables, dataDir } = ctx;
  const operations: WriteOperation[] = [];
  const deletePaths: string[] = [];

  for (const sessionId of getSessionIdsToProcess(ctx, transcriptsBySession)) {
    const session = tables.sessions?.[sessionId];
    if (!session) continue;

    const sessionDir = buildSessionPath(
      dataDir,
      sessionId,
      session.folder_id ?? "",
    );
    const path = [sessionDir, SESSION_TRANSCRIPT_FILE].join(sep());
    const transcripts = transcriptsBySession.get(sessionId) ?? [];

    if (transcripts.length === 0) {
      deletePaths.push(path);
      continue;
    }

    const content: TranscriptJson = { transcripts };
    operations.push({
      type: "write-json",
      path,
      content,
    });
  }

  if (deletePaths.length > 0) {
    operations.push({ type: "delete", paths: deletePaths });
  }

  return operations;
}

function getSessionIdsToProcess(
  ctx: BuildContext,
  transcriptsBySession: Map<string, TranscriptWithData[]>,
): string[] {
  if (ctx.changedSessionIds) {
    return [...ctx.changedSessionIds];
  }

  const sessionIds = new Set(Object.keys(ctx.tables.sessions ?? {}));
  for (const sessionId of transcriptsBySession.keys()) {
    sessionIds.add(sessionId);
  }

  return [...sessionIds];
}
