import { sep } from "@tauri-apps/api/path";

import type {
  ParticipantData,
  SessionKeyFactsData,
  SessionMetaJson,
} from "~/store/tinybase/persister/session/types";
import {
  buildSessionPath,
  iterateTableRows,
  SESSION_META_FILE,
  type TablesContent,
  type WriteOperation,
} from "~/store/tinybase/persister/shared";
import type { Store } from "~/store/tinybase/store/main";

type SessionMetaWithFolder = {
  meta: SessionMetaJson;
  folderPath: string;
};

function tryParseJson(
  value: string | undefined,
): Record<string, unknown> | undefined {
  if (!value) return undefined;
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
}

type MetaItem = [SessionMetaJson, string];

type BuildContext = {
  tables: TablesContent;
  dataDir: string;
  changedSessionIds?: Set<string>;
};

export function tablesToSessionMetaMap(
  store: Store,
): Map<string, SessionMetaWithFolder> {
  const tables = store.getTables() as TablesContent;

  const participantsBySession = buildParticipantMap(tables);
  const keyFactsBySession = buildKeyFactsMap(tables);
  const tagsBySession = buildTagMap(tables);

  const result = new Map<string, SessionMetaWithFolder>();

  for (const session of iterateTableRows(tables, "sessions")) {
    result.set(session.id, {
      meta: {
        id: session.id,
        user_id: session.user_id ?? "",
        created_at: session.created_at ?? "",
        title: session.title ?? "",
        event: tryParseJson(session.event_json),
        participants: participantsBySession.get(session.id) ?? [],
        key_facts: keyFactsBySession.get(session.id),
        tags: tagsBySession.get(session.id),
      },
      folderPath: session.folder_id ?? "",
    });
  }

  return result;
}

export function buildSessionSaveOps(
  _store: Store,
  tables: TablesContent,
  dataDir: string,
  changedSessionIds?: Set<string>,
): WriteOperation[] {
  const ctx: BuildContext = { tables, dataDir, changedSessionIds };

  const items = collectSessionMetas(ctx);

  return buildOperations(items);
}

function collectSessionMetas(ctx: BuildContext): MetaItem[] {
  const { tables, dataDir, changedSessionIds } = ctx;

  const participantsBySession = buildParticipantMap(tables);
  const keyFactsBySession = buildKeyFactsMap(tables);
  const tagsBySession = buildTagMap(tables);

  return iterateTableRows(tables, "sessions")
    .filter(
      (session) => !changedSessionIds || changedSessionIds.has(session.id),
    )
    .map((session) => {
      const meta: SessionMetaJson = {
        id: session.id,
        user_id: session.user_id ?? "",
        created_at: session.created_at ?? "",
        title: session.title ?? "",
        event: tryParseJson(session.event_json),
        participants: participantsBySession.get(session.id) ?? [],
        key_facts: keyFactsBySession.get(session.id),
        tags: tagsBySession.get(session.id),
      };

      const sessionDir = buildSessionPath(
        dataDir,
        session.id,
        session.folder_id ?? "",
      );
      const path = [sessionDir, SESSION_META_FILE].join(sep());

      return [meta, path] as MetaItem;
    });
}

function buildParticipantMap(
  tables: TablesContent,
): Map<string, ParticipantData[]> {
  const result = new Map<string, ParticipantData[]>();

  for (const p of iterateTableRows(tables, "mapping_session_participant")) {
    if (!p.session_id) continue;

    const list = result.get(p.session_id) ?? [];
    list.push({
      id: p.id,
      user_id: p.user_id,
      session_id: p.session_id,
      human_id: p.human_id,
      source: p.source,
    });
    result.set(p.session_id, list);
  }

  return result;
}

function buildKeyFactsMap(
  tables: TablesContent,
): Map<string, SessionKeyFactsData> {
  const result = new Map<string, SessionKeyFactsData>();

  for (const row of iterateTableRows(tables, "session_key_facts")) {
    if (!row.session_id || !row.content || !row.source_hash) {
      continue;
    }

    result.set(row.session_id, {
      id: row.id,
      user_id: row.user_id,
      session_id: row.session_id,
      created_at: row.created_at,
      updated_at: row.updated_at,
      content: row.content,
      source_hash: row.source_hash,
    });
  }

  return result;
}

function buildTagMap(tables: TablesContent): Map<string, string[] | undefined> {
  const result = new Map<string, string[] | undefined>();

  const tags = tables.tags ?? {};

  for (const mapping of iterateTableRows(tables, "mapping_tag_session")) {
    if (!mapping.session_id || !mapping.tag_id) continue;

    const tag = tags[mapping.tag_id];
    if (!tag?.name) continue;

    const list = result.get(mapping.session_id) ?? [];
    list.push(tag.name);
    result.set(mapping.session_id, list);
  }

  return result;
}

function buildOperations(items: MetaItem[]): WriteOperation[] {
  return items.map(([meta, path]) => ({
    type: "write-json" as const,
    path,
    content: meta,
  }));
}
