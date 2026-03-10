import { disable, enable } from "@tauri-apps/plugin-autostart";
import { useEffect } from "react";
import { createBroadcastChannelSynchronizer } from "tinybase/synchronizers/synchronizer-broadcast-channel/with-schemas";
import * as _UI from "tinybase/ui-react/with-schemas";
import {
  createMergeableStore,
  createQueries,
  type MergeableStore,
  type TablesSchema,
  type ValuesSchema,
} from "tinybase/with-schemas";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as detectCommands } from "@hypr/plugin-detect";
import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { registerSaveHandler } from "./save";

import { useSettingsPersister } from "~/store/tinybase/persister/settings";

export const STORE_ID = "settings";

export const SETTINGS_MAPPING = {
  values: {
    autostart: {
      type: "boolean",
      path: ["general", "autostart"],
      default: false as boolean,
    },
    save_recordings: {
      type: "boolean",
      path: ["general", "save_recordings"],
      default: true as boolean,
    },
    notification_event: {
      type: "boolean",
      path: ["notification", "event"],
      default: true as boolean,
    },
    notification_detect: {
      type: "boolean",
      path: ["notification", "detect"],
      default: true as boolean,
    },
    respect_dnd: {
      type: "boolean",
      path: ["notification", "respect_dnd"],
      default: false as boolean,
    },
    telemetry_consent: {
      type: "boolean",
      path: ["general", "telemetry_consent"],
      default: true as boolean,
    },
    ai_language: {
      type: "string",
      path: ["language", "ai_language"],
      default: "en" as string,
    },
    spoken_languages: {
      type: "string",
      path: ["language", "spoken_languages"],
      default: '["en"]' as string,
    },
    ignored_platforms: {
      type: "string",
      path: ["notification", "ignored_platforms"],
      default: "[]" as string,
    },
    mic_active_threshold: {
      type: "number",
      path: ["notification", "mic_active_threshold"],
      default: 15 as number,
    },
    current_llm_provider: {
      type: "string",
      path: ["ai", "current_llm_provider"],
    },
    current_llm_model: {
      type: "string",
      path: ["ai", "current_llm_model"],
    },
    current_stt_provider: {
      type: "string",
      path: ["ai", "current_stt_provider"],
    },
    current_stt_model: {
      type: "string",
      path: ["ai", "current_stt_model"],
    },
    cactus_cloud_handoff: {
      type: "boolean",
      path: ["cactus", "cloud_handoff"],
    },
    cactus_min_chunk_sec: {
      type: "number",
      path: ["cactus", "min_chunk_sec"],
    },
    timezone: {
      type: "string",
      path: ["general", "timezone"],
    },
    week_start: {
      type: "string",
      path: ["general", "week_start"],
    },
    selected_template_id: {
      type: "string",
      path: ["general", "selected_template_id"],
    },
  },
  tables: {
    ai_providers: {
      schema: {
        type: { type: "string" },
        base_url: { type: "string" },
        api_key: { type: "string" },
      },
    },
  },
} as const;

export type SettingsValueKey = keyof typeof SETTINGS_MAPPING.values;

type ValueType = "boolean" | "string" | "number";
type ValueMapping = {
  type: ValueType;
  path: readonly [string, string];
  default?: boolean | string | number;
};

type DeriveValuesSchema<T extends Record<string, ValueMapping>> = {
  [K in keyof T]: T[K] extends { default: infer D }
    ? { type: T[K]["type"]; default: D }
    : { type: T[K]["type"] };
};

export const SCHEMA = {
  value: Object.fromEntries(
    Object.entries(SETTINGS_MAPPING.values).map(([key, config]) => [
      key,
      "default" in config
        ? { type: config.type, default: config.default }
        : { type: config.type },
    ]),
  ) as DeriveValuesSchema<
    typeof SETTINGS_MAPPING.values
  > satisfies ValuesSchema,
  table: Object.fromEntries(
    Object.entries(SETTINGS_MAPPING.tables).map(([key, config]) => [
      key,
      config.schema,
    ]),
  ) as {
    ai_providers: typeof SETTINGS_MAPPING.tables.ai_providers.schema;
  } satisfies TablesSchema,
} as const;

export type Schemas = [typeof SCHEMA.table, typeof SCHEMA.value];

const {
  useCreateMergeableStore,
  useCreateSynchronizer,
  useCreateQueries,
  useProvideStore,
  useProvidePersister,
  useProvideSynchronizer,
  useProvideQueries,
} = _UI as _UI.WithSchemas<Schemas>;

export const UI = _UI as _UI.WithSchemas<Schemas>;
export type Store = MergeableStore<Schemas>;

export const QUERIES = {
  llmProviders: "llmProviders",
  sttProviders: "sttProviders",
} as const;

export const StoreComponent = () => {
  const store = useCreateMergeableStore(() =>
    createMergeableStore()
      .setTablesSchema(SCHEMA.table)
      .setValuesSchema(SCHEMA.value),
  );

  const persister = useSettingsPersister(store);

  useEffect(() => {
    if (!persister) {
      return;
    }

    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    return registerSaveHandler("settings", async () => {
      await persister.save();
    });
  }, [persister]);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    return registerSettingsListeners(store);
  }, [store]);

  const synchronizer = useCreateSynchronizer(store, async (store) =>
    createBroadcastChannelSynchronizer(store, "hypr-sync-settings").startSync(),
  );

  const queries = useCreateQueries(store, (store) =>
    createQueries(store)
      .setQueryDefinition(
        QUERIES.llmProviders,
        "ai_providers",
        ({ select, where }) => {
          select("type");
          select("base_url");
          select("api_key");
          where((getCell) => getCell("type") === "llm");
        },
      )
      .setQueryDefinition(
        QUERIES.sttProviders,
        "ai_providers",
        ({ select, where }) => {
          select("type");
          select("base_url");
          select("api_key");
          where((getCell) => getCell("type") === "stt");
        },
      ),
  );

  useProvideStore(STORE_ID, store);
  useProvideQueries(STORE_ID, queries!);
  useProvidePersister(STORE_ID, persister);
  useProvideSynchronizer(STORE_ID, synchronizer);

  return null;
};

export const SETTINGS_VALUE_KEYS = Object.keys(
  SETTINGS_MAPPING.values,
) as (keyof typeof SETTINGS_MAPPING.values)[];

type ValueTypeMap = { boolean: boolean; string: string; number: number };
type SettingsValueType<K extends SettingsValueKey> =
  ValueTypeMap[(typeof SETTINGS_MAPPING.values)[K]["type"]];

type SettingsListeners = {
  [K in SettingsValueKey]?: (
    store: Store,
    newValue: SettingsValueType<K>,
  ) => void;
};

const SETTINGS_LISTENERS: SettingsListeners = {
  autostart: (_store, newValue) => {
    if (newValue) {
      enable().catch(console.error);
    } else {
      disable().catch(console.error);
    }
  },
  respect_dnd: (_store, newValue) => {
    detectCommands.setRespectDoNotDisturb(newValue).catch(console.error);
  },
  ignored_platforms: (_store, newValue) => {
    try {
      const parsed = JSON.parse(newValue);
      detectCommands.setIgnoredBundleIds(parsed).catch(console.error);
    } catch {}
  },
  mic_active_threshold: (_store, newValue) => {
    detectCommands.setMicActiveThreshold(newValue).catch(console.error);
  },
  current_stt_provider: (store) => {
    const provider = store.getValue("current_stt_provider") as
      | string
      | undefined;
    const model = store.getValue("current_stt_model") as string | undefined;

    if (provider === "hyprnote" && model && model !== "cloud") {
      localSttCommands.startServer(model as LocalModel).catch(console.error);
    }
  },
  current_stt_model: (store) => {
    const provider = store.getValue("current_stt_provider") as
      | string
      | undefined;
    const model = store.getValue("current_stt_model") as string | undefined;

    if (provider === "hyprnote" && model && model !== "cloud") {
      localSttCommands.startServer(model as LocalModel).catch(console.error);
    } else {
      localSttCommands.stopServer(null).catch(console.error);
    }
  },
  telemetry_consent: (_store, newValue) => {
    analyticsCommands.setDisabled(!newValue).catch(console.error);
  },
};

function registerSettingsListeners(store: Store): () => void {
  const cleanups: string[] = [];

  for (const [key, handler] of Object.entries(SETTINGS_LISTENERS) as [
    SettingsValueKey,
    (store: Store, newValue: any) => void,
  ][]) {
    cleanups.push(
      store.addValueListener(key, (store, _key, newValue) => {
        handler(store, newValue);
      }),
    );
  }

  return () => {
    for (const id of cleanups) {
      store.delListener(id);
    }
  };
}
