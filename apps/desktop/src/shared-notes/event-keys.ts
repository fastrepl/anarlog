import type { Session, SupabaseClient } from "@supabase/supabase-js";
import { useMemo } from "react";

import { sharedNoteEventKey } from "./event-key";

import { useAuth } from "~/auth";
import {
  SESSION_EVENT_MATCH,
  SESSION_EVENT_ORDER,
} from "~/calendar/session-event-match";
import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { useDurableSharedNotes } from "~/shared-notes/cache";

const SETTING_ID = "shared_note_event_keys";
const EMPTY_KEYS: ReadonlyMap<string, string> = new Map();
const EMPTY_IDS: string[] = [];
const MATCHED_EVENT_SELECT = `
  SELECT matched.tracking_id_event, matched.started_at
  FROM sessions AS session
  JOIN events AS matched ON matched.id = (
    SELECT event.id
    FROM events AS event
    WHERE ${SESSION_EVENT_MATCH}
    ORDER BY ${SESSION_EVENT_ORDER}
    LIMIT 1
  )
`;
const EMPTY_LOCAL_KEYS: ReadonlySet<string> = new Set();

type AppSettingSqlRow = { value_json: string | null };
type EventSqlRow = {
  tracking_id_event: string | null;
  started_at: string | null;
};

export async function syncSharedNoteEventKeys(
  supabase: SupabaseClient,
  session: Session,
  signal: AbortSignal,
): Promise<void> {
  const { data, error }: { data: unknown; error: unknown } = await supabase
    .rpc("list_my_session_share_event_keys")
    .setHeader("Authorization", `${session.token_type} ${session.access_token}`)
    .abortSignal(signal);
  if (error) throw error;
  if (!Array.isArray(data)) {
    throw new Error("invalid shared-note event keys");
  }
  const keys: Record<string, string> = {};
  for (const row of data) {
    if (
      row &&
      typeof row === "object" &&
      typeof row.share_id === "string" &&
      typeof row.event_key === "string" &&
      row.event_key
    ) {
      keys[row.share_id] = row.event_key;
    }
  }
  signal.throwIfAborted();
  const valueJson = JSON.stringify({
    viewer_user_id: session.user.id,
    keys,
  });
  await enqueueDatabaseWrite(`app-setting:${SETTING_ID}`, async () => {
    await executeTransaction([
      {
        sql: `
          INSERT INTO app_settings (id, value_json, updated_at)
          VALUES (?, ?, ?)
          ON CONFLICT(id) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at = excluded.updated_at
        `,
        params: [SETTING_ID, valueJson, new Date().toISOString()],
      },
    ]);
  });
}

export function parseSharedNoteEventKeys(
  valueJson: string | null | undefined,
  viewerUserId: string | null | undefined,
): ReadonlyMap<string, string> {
  if (!valueJson || !viewerUserId) return EMPTY_KEYS;
  try {
    const parsed = JSON.parse(valueJson) as unknown;
    if (
      !parsed ||
      typeof parsed !== "object" ||
      !("viewer_user_id" in parsed) ||
      parsed.viewer_user_id !== viewerUserId ||
      !("keys" in parsed) ||
      !parsed.keys ||
      typeof parsed.keys !== "object"
    ) {
      return EMPTY_KEYS;
    }
    return new Map(
      Object.entries(parsed.keys).filter(
        (entry): entry is [string, string] =>
          typeof entry[1] === "string" && entry[1].length > 0,
      ),
    );
  } catch {
    return EMPTY_KEYS;
  }
}

export function useSharedNoteEventKeys(
  viewerUserId: string | null | undefined,
): ReadonlyMap<string, string> {
  const { data = EMPTY_KEYS } = useLiveQuery<
    AppSettingSqlRow,
    ReadonlyMap<string, string>
  >({
    sql: `SELECT value_json FROM app_settings WHERE id = ? AND ? <> ''`,
    params: [SETTING_ID, viewerUserId ?? ""],
    mapRows: (rows) =>
      parseSharedNoteEventKeys(rows[0]?.value_json, viewerUserId),
  });
  return data;
}

export function sharedNoteIdsForEventKey(
  eventKey: string,
  eventKeys: ReadonlyMap<string, string>,
  notes: readonly { shareId: string; manageAccess: boolean }[],
): string[] {
  if (!eventKey) return EMPTY_IDS;
  const received = new Set(
    notes.filter((note) => !note.manageAccess).map((note) => note.shareId),
  );
  const ids = [...eventKeys]
    .filter(([shareId, key]) => key === eventKey && received.has(shareId))
    .map(([shareId]) => shareId)
    .sort();
  return ids.length > 0 ? ids : EMPTY_IDS;
}

export function useSessionSharedNoteIds(sessionId: string): string[] {
  const { session } = useAuth();
  const viewerUserId = session?.user.id ?? null;
  const eventKeys = useSharedNoteEventKeys(viewerUserId);
  const notes = useDurableSharedNotes(viewerUserId);
  const { data: sessionEventKey = "" } = useLiveQuery<EventSqlRow, string>({
    sql: `
      ${MATCHED_EVENT_SELECT}
      WHERE session.id = ?
    `,
    params: [sessionId],
    mapRows: (rows) =>
      sharedNoteEventKey(rows[0]?.tracking_id_event, rows[0]?.started_at),
  });
  return useMemo(
    () => sharedNoteIdsForEventKey(sessionEventKey, eventKeys, notes),
    [sessionEventKey, eventKeys, notes],
  );
}

export function useLocalSessionEventKeys(): ReadonlySet<string> {
  const { data = EMPTY_LOCAL_KEYS } = useLiveQuery<
    EventSqlRow,
    ReadonlySet<string>
  >({
    sql: `
      ${MATCHED_EVENT_SELECT}
      WHERE session.deleted_at IS NULL
    `,
    params: [],
    mapRows: (rows) =>
      new Set(
        rows
          .map((row) =>
            sharedNoteEventKey(row.tracking_id_event, row.started_at),
          )
          .filter(Boolean),
      ),
  });
  return data;
}
