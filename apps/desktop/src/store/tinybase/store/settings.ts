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
import { commands as localSttCommands } from "@hypr/plugin-local-stt";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { registerSaveHandler } from "./save";

import { useSettingsPersister } from "~/store/tinybase/persister/settings";
import {
  isConfiguredSttModel,
  isHyprnoteLocalSttModel,
} from "~/stt/capabilities";

export const STORE_ID = "settings";

export const SETTINGS_MAPPING = {
  values: {
    autostart: {
      type: "boolean",
      path: ["general", "autostart"],
      default: false as boolean,
    },
    auto_stop_meetings: {
      type: "boolean",
      path: ["general", "auto_stop_meetings"],
      default: true as boolean,
    },
    auto_start_scheduled_meetings: {
      type: "boolean",
      path: ["general", "auto_start_scheduled_meetings"],
      default: true as boolean,
    },
    floating_bar_enabled: {
      type: "boolean",
      path: ["general", "floating_bar_enabled"],
      default: true as boolean,
    },
    sidebar_timeline_enabled: {
      type: "boolean",
      path: ["general", "sidebar_timeline_enabled"],
      default: false as boolean,
    },
    theme: {
      type: "string",
      path: ["general", "theme"],
      default: "system" as string,
    },
    save_recordings: {
      type: "boolean",
      path: ["general", "save_recordings"],
      default: true as boolean,
    },
    audio_retention: {
      type: "string",
      path: ["general", "audio_retention"],
      default: "forever" as string,
      schemaDefault: false,
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
    // Actual values populated via persister load; defaults here are for type inference.
    ai_language: {
      type: "string",
      path: ["language", "ai_language"],
      default: "" as string,
    },
    spoken_languages: {
      type: "string",
      path: ["language", "spoken_languages"],
      default: "[]" as string,
    },
    ignored_platforms: {
      type: "string",
      path: ["notification", "ignored_platforms"],
      default: "[]" as string,
    },
    included_platforms: {
      type: "string",
      path: ["notification", "included_platforms"],
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
    todo_linear_filter: {
      type: "string",
      path: ["todo", "linear_filter"],
      default: "" as string,
    },
    todo_github_repository: {
      type: "string",
      path: ["todo", "github_repository"],
      default: "" as string,
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
  schemaDefault?: boolean;
};

type DeriveValuesSchema<T extends Record<string, ValueMapping>> = {
  [K in keyof T]: T[K] extends { default: infer D }
    ? T[K] extends { schemaDefault: false }
      ? { type: T[K]["type"] }
      : { type: T[K]["type"]; default: D }
    : { type: T[K]["type"] };
};

export const SCHEMA = {
  value: Object.fromEntries(
    Object.entries(SETTINGS_MAPPING.values).map(([key, config]) => [
      key,
      "default" in config &&
      !("schemaDefault" in config && config.schemaDefault === false)
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

function clearInvalidSttModel(store: Store) {
  const provider = store.getValue("current_stt_provider") as string | undefined;
  const model = store.getValue("current_stt_model") as string | undefined;

  if (
    provider === "hyprnote" &&
    model &&
    !isConfiguredSttModel(provider, model)
  ) {
    store.delValue("current_stt_model");
    return true;
  }

  return false;
}

function syncLocalSttServer(store: Store) {
  if (clearInvalidSttModel(store)) {
    localSttCommands.stopServer(null).catch(console.error);
    return;
  }

  const provider = store.getValue("current_stt_provider") as string | undefined;
  const model = store.getValue("current_stt_model") as string | undefined;

  if (isHyprnoteLocalSttModel(provider, model)) {
    localSttCommands.startServer(model).catch(console.error);
  } else {
    localSttCommands.stopServer(null).catch(console.error);
  }
}

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
  included_platforms: (_store, newValue) => {
    try {
      const parsed = JSON.parse(newValue);
      detectCommands.setIncludedBundleIds(parsed).catch(console.error);
    } catch {}
  },
  mic_active_threshold: (_store, newValue) => {
    detectCommands.setMicActiveThreshold(newValue).catch(console.error);
  },
  current_stt_provider: (store) => syncLocalSttServer(store),
  current_stt_model: (store) => syncLocalSttServer(store),
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

  clearInvalidSttModel(store);

  return () => {
    for (const id of cleanups) {
      store.delListener(id);
    }
  };
}
