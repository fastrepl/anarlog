import {
  type ChangedTables,
  getChangedIds,
  iterateTableRows,
  SESSION_META_FILE,
  SESSION_NOTE_EXTENSION,
  SESSION_TRANSCRIPT_FILE,
  type TablesContent,
} from "~/store/tinybase/persister/shared";

export function parseSessionIdFromPath(path: string): string | null {
  const parts = path.split("/");
  const sessionsIndex = parts.indexOf("sessions");
  if (sessionsIndex === -1) {
    return null;
  }

  const filename = parts[parts.length - 1];
  const isSessionFile =
    filename === SESSION_META_FILE ||
    filename === SESSION_TRANSCRIPT_FILE ||
    filename?.endsWith(SESSION_NOTE_EXTENSION);

  if (isSessionFile && parts.length >= 2) {
    return parts[parts.length - 2] || null;
  }

  return null;
}

export type SessionChangeResult = {
  changedSessionIds: Set<string>;
  hasUnresolvedDeletions: boolean;
};

export type SessionSaveScope = {
  session: boolean;
  transcript: boolean;
  note: boolean;
};

const ALL_SESSION_SAVE_SCOPES: SessionSaveScope = {
  session: true,
  transcript: true,
  note: true,
};

const SESSION_META_CELLS = new Set([
  "user_id",
  "created_at",
  "title",
  "event_json",
  "folder_id",
]);
const SESSION_NOTE_CELLS = new Set(["raw_md", "folder_id"]);

export function getChangedSessionIds(
  tables: TablesContent,
  changedTables: ChangedTables,
): SessionChangeResult | undefined {
  const result = getChangedIds(tables, changedTables, [
    { table: "sessions", extractId: (id) => id },
    {
      table: "mapping_session_participant",
      extractId: (id, tables) =>
        tables.mapping_session_participant?.[id]?.session_id,
    },
    {
      table: "mapping_tag_session",
      extractId: (id, tables) => tables.mapping_tag_session?.[id]?.session_id,
    },
    {
      table: "transcripts",
      extractId: (id, tables) => tables.transcripts?.[id]?.session_id,
    },
    {
      table: "enhanced_notes",
      extractId: (id, tables) => tables.enhanced_notes?.[id]?.session_id,
    },
  ]);

  const changedSessionIds = new Set(result?.changedIds ?? []);
  addTagChangeSessionIds(tables, changedTables, changedSessionIds);

  if (changedSessionIds.size === 0 && !result?.hasUnresolvedDeletions) {
    return undefined;
  }

  return {
    changedSessionIds,
    hasUnresolvedDeletions: result?.hasUnresolvedDeletions ?? false,
  };
}

function addTagChangeSessionIds(
  tables: TablesContent,
  changedTables: ChangedTables,
  changedSessionIds: Set<string>,
) {
  if (!hasTableChange(changedTables, "tags")) {
    return;
  }

  const changedTags = changedTables.tags;
  const changedTagIds = changedTags ? new Set(Object.keys(changedTags)) : null;

  for (const mapping of iterateTableRows(tables, "mapping_tag_session")) {
    if (!mapping.session_id || !mapping.tag_id) {
      continue;
    }

    if (!changedTagIds || changedTagIds.has(mapping.tag_id)) {
      changedSessionIds.add(mapping.session_id);
    }
  }
}

export function getSessionSaveScope(
  changedTables?: ChangedTables,
): SessionSaveScope {
  if (!changedTables) {
    return ALL_SESSION_SAVE_SCOPES;
  }

  const sessionPathChanged = hasSessionCellChange(
    changedTables,
    new Set(["folder_id"]),
  );

  return {
    session:
      hasSessionCellChange(changedTables, SESSION_META_CELLS) ||
      hasTableChange(changedTables, "mapping_session_participant") ||
      hasTableChange(changedTables, "mapping_tag_session") ||
      hasTableChange(changedTables, "tags"),
    transcript:
      sessionPathChanged || hasTableChange(changedTables, "transcripts"),
    note:
      hasSessionCellChange(changedTables, SESSION_NOTE_CELLS) ||
      hasTableChange(changedTables, "enhanced_notes"),
  };
}

function hasTableChange(
  changedTables: ChangedTables,
  table: keyof ChangedTables,
): boolean {
  return Object.prototype.hasOwnProperty.call(changedTables, table);
}

function hasSessionCellChange(
  changedTables: ChangedTables,
  cellIds: Set<string>,
): boolean {
  const changedSessions = changedTables.sessions;
  if (!hasTableChange(changedTables, "sessions")) {
    return false;
  }
  if (!changedSessions) {
    return true;
  }

  return Object.values(changedSessions).some((rowChange) => {
    const changedCellIds = getChangedCellIds(rowChange);
    if (!changedCellIds) {
      return true;
    }

    for (const cellId of cellIds) {
      if (changedCellIds.has(cellId)) {
        return true;
      }
    }

    return false;
  });
}

function getChangedCellIds(rowChange: unknown): Set<string> | null {
  const row =
    Array.isArray(rowChange) && rowChange.length > 0 ? rowChange[0] : rowChange;

  if (!row || typeof row !== "object" || Array.isArray(row)) {
    return null;
  }

  const cellIds = Object.keys(row);
  return cellIds.length > 0 ? new Set(cellIds) : null;
}
