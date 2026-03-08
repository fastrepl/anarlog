import { createMergeableStore } from "tinybase/with-schemas";

import { SCHEMA } from "@hypr/store";

import {
  SCHEMA as SETTINGS_SCHEMA,
  type Store as SettingsStore,
} from "~/store/tinybase/store/settings";

export const MOCK_DATA_DIR = "/mock/data/dir/hyprnote";

export const TEST_UUID_1 = "550e8400-e29b-41d4-a716-446655440000";
export const TEST_UUID_2 = "550e8400-e29b-41d4-a716-446655440001";

export function createTestMainStore() {
  return createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value);
}

export function createTestSettingsStore(): SettingsStore {
  return createMergeableStore()
    .setTablesSchema(SETTINGS_SCHEMA.table)
    .setValuesSchema(SETTINGS_SCHEMA.value) as SettingsStore;
}
