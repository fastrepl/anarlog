import type { Session, SupabaseClient } from "@supabase/supabase-js";

import { sharedNoteEventKey } from "./event-key";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

const SETTING_ID = "shared_note_event_keys";
const EMPTY_KEYS: ReadonlyMap<string, string> = new Map();
const EMPTY_LOCAL_KEYS: ReadonlySet<string> = new Set();

type AppSettingSqlRow = { value_json: string | null };
type LocalEventSqlRow = { tracking_id_event: string; started_at: string };

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
    sql: `SELECT value_json FROM app_settings WHERE id = ?`,
    params: [SETTING_ID],
    mapRows: (rows) =>
      parseSharedNoteEventKeys(rows[0]?.value_json, viewerUserId),
  });
  return data;
}

export function shareIdForEventKey(
  eventKeys: ReadonlyMap<string, string>,
  eventKey: string,
): string | null {
  if (!eventKey) return null;
  for (const [shareId, key] of eventKeys) {
    if (key === eventKey) return shareId;
  }
  return null;
}

export function useLocalEventKeys(
  eventKeys: ReadonlyMap<string, string>,
): ReadonlySet<string> {
  const trackingIds = Array.from(
    new Set(
      Array.from(eventKeys.values(), (key) =>
        key.slice(0, key.lastIndexOf("|")),
      ),
    ),
  ).sort();
  const { data = EMPTY_LOCAL_KEYS } = useLiveQuery<
    LocalEventSqlRow,
    ReadonlySet<string>
  >({
    sql: `
      SELECT tracking_id_event, started_at
      FROM events
      WHERE tracking_id_event IN (SELECT value FROM json_each(?))
    `,
    params: [JSON.stringify(trackingIds)],
    mapRows: (rows) =>
      new Set(
        rows.map((row) =>
          sharedNoteEventKey(row.tracking_id_event, row.started_at),
        ),
      ),
  });
  return data;
}
