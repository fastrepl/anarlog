import type { IncomingEvent } from "../../fetch/types";

import {
  LEGACY_MAIN_VALUES_ID,
  LEGACY_SETTINGS_ID,
} from "~/settings/legacy-snapshots";

export function migrateIgnoredEventIds(
  events: Pick<IncomingEvent, "tracking_id_event" | "legacy_tracking_ids">[],
  now: string,
) {
  const aliases = new Map<string, string>();
  const ambiguous = new Set<string>();
  for (const event of events) {
    for (const legacyId of event.legacy_tracking_ids ?? []) {
      if (legacyId === event.tracking_id_event) continue;
      const current = aliases.get(legacyId);
      if (current && current !== event.tracking_id_event)
        ambiguous.add(legacyId);
      aliases.set(legacyId, event.tracking_id_event);
    }
  }
  for (const legacyId of ambiguous) aliases.delete(legacyId);
  if (aliases.size === 0) return [];

  return [
    {
      sql: `
      WITH current AS (
        SELECT COALESCE(
          (SELECT value_json FROM app_settings WHERE id = 'ignored_events'),
          (SELECT CASE WHEN json_valid(value_json) THEN json_extract(value_json, '$.ignored_events') END
            FROM app_settings WHERE id = ?),
          (SELECT CASE WHEN json_valid(value_json) THEN json_extract(value_json, '$.ignored_events') END
            FROM app_settings WHERE id = ?),
          '[]'
        ) AS value_json
      ), aliases AS (
        SELECT json_extract(value, '$[0]') AS old_id, json_extract(value, '$[1]') AS new_id
        FROM json_each(?)
      ), entries AS (
        SELECT item.value, aliases.new_id
        FROM current, json_each(CASE WHEN json_valid(current.value_json) THEN current.value_json ELSE '[]' END) AS item
        LEFT JOIN aliases ON aliases.old_id = json_extract(item.value, '$.tracking_id')
      )
      INSERT INTO app_settings (id, value_json, updated_at)
      SELECT 'ignored_events', (
        SELECT json_group_array(json(CASE WHEN new_id IS NULL THEN value
          ELSE json_set(value, '$.tracking_id', new_id) END)) FROM entries
      ), ?
      WHERE EXISTS (SELECT 1 FROM entries WHERE new_id IS NOT NULL)
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at
    `,
      params: [
        LEGACY_MAIN_VALUES_ID,
        LEGACY_SETTINGS_ID,
        JSON.stringify([...aliases]),
        now,
      ],
    },
  ];
}
