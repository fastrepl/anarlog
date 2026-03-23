import { SCHEMA } from "@hypr/store";

import type { TablesContent } from "~/store/tinybase/persister/shared";

export const SESSION_TABLES = [
  "sessions",
  "mapping_session_participant",
  "tags",
  "mapping_tag_session",
  "transcripts",
  "enhanced_notes",
] as const satisfies readonly (keyof typeof SCHEMA.table)[];

type SessionTables = (typeof SESSION_TABLES)[number];

export type LoadedSessionData = Pick<Required<TablesContent>, SessionTables>;

export function createEmptyLoadedSessionData(): LoadedSessionData {
  return Object.fromEntries(
    SESSION_TABLES.map((table) => [table, {}]),
  ) as LoadedSessionData;
}
