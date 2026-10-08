import type { Session, SupabaseClient } from "@supabase/supabase-js";

import { sharedNoteEventKey } from "./event-key";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

const SETTING_ID = "shared_note_event_keys";
const EMPTY_KEYS: ReadonlyMap<string, string> = new Map();

type AppSettingSqlRow = { value_json: string | null };

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

export function withoutSharedEvents<
  T extends {
    tracking_id_event?: string | null;
    started_at?: string | null;
  },
>(
  table: Record<string, T> | null | undefined,
  eventKeys: ReadonlyMap<string, string>,
): Record<string, T> | null | undefined {
  if (!table || eventKeys.size === 0) return table;
  const sharedKeys = new Set(eventKeys.values());
  const entries = Object.entries(table);
  const visible = entries.filter(
    ([, event]) =>
      !sharedKeys.has(
        sharedNoteEventKey(event.tracking_id_event, event.started_at),
      ),
  );
  return visible.length === entries.length
    ? table
    : Object.fromEntries(visible);
}
