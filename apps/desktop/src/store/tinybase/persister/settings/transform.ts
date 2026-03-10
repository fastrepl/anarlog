import type { Content } from "tinybase/with-schemas";

import type { Schemas, Store } from "~/store/tinybase/store/settings";
import { SETTINGS_MAPPING } from "~/store/tinybase/store/settings";

type ProviderData = { base_url: string; api_key: string };
type ProviderRow = { type: "llm" | "stt"; base_url: string; api_key: string };

const JSON_ARRAY_FIELDS = new Set([
  "spoken_languages",
  "ignored_platforms",
  "ignored_recurring_series",
]);

function getByPath(obj: unknown, path: readonly [string, string]): unknown {
  const section = (obj as Record<string, unknown>)?.[path[0]];
  return (section as Record<string, unknown>)?.[path[1]];
}

function setByPath(
  obj: Record<string, Record<string, unknown>>,
  path: readonly [string, string],
  value: unknown,
): void {
  obj[path[0]] ??= {};
  obj[path[0]][path[1]] = value;
}

function toStoreValue(key: string, value: unknown): unknown {
  if (!JSON_ARRAY_FIELDS.has(key)) {
    return value;
  }
  if (Array.isArray(value)) {
    return JSON.stringify(value);
  }
  if (typeof value === "string") {
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return value;
      }
    } catch {}
  }
  return value;
}

function fromStoreValue(key: string, value: unknown): unknown {
  if (!JSON_ARRAY_FIELDS.has(key)) {
    return value;
  }

  if (typeof value === "string") {
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return parsed;
      }
    } catch {}

    if (value.includes(",")) {
      return value
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
    }
  }
  return value;
}

function settingsToStoreValues(settings: unknown): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const [key, config] of Object.entries(SETTINGS_MAPPING.values)) {
    let value = getByPath(settings, config.path);

    if (value === undefined) {
      if (key === "ai_language") {
        value = getByPath(settings, ["general", "ai_language"]);
      } else if (key === "spoken_languages") {
        value = getByPath(settings, ["general", "spoken_languages"]);
      }
    }

    if (value !== undefined) {
      values[key] = toStoreValue(key, value);
    }
  }
  return values;
}

function settingsToProviderRows(
  settings: unknown,
): Record<string, ProviderRow> {
  const rows: Record<string, ProviderRow> = {};
  const ai = (settings as Record<string, unknown>)?.ai as
    | Record<string, unknown>
    | undefined;

  for (const providerType of ["llm", "stt"] as const) {
    const providers = (ai?.[providerType] ?? {}) as Record<
      string,
      ProviderData
    >;
    for (const [id, data] of Object.entries(providers)) {
      if (data) {
        rows[`${providerType}:${id}`] = {
          type: providerType,
          base_url: data.base_url ?? "",
          api_key: data.api_key ?? "",
        };
      }
    }
  }
  return rows;
}

export function storeValuesToSettings(
  values: Record<string, unknown>,
): Record<string, unknown> {
  const result: Record<string, Record<string, unknown>> = {
    ai: { llm: {}, stt: {} },
    notification: {},
    general: {},
    language: {},
    cactus: {},
  };

  for (const [key, config] of Object.entries(SETTINGS_MAPPING.values)) {
    const value = values[key];
    if (value === undefined) {
      continue;
    }
    if ("default" in config && value === config.default) {
      continue;
    }
    setByPath(result, config.path, fromStoreValue(key, value));
  }

  return result;
}

function providerRowsToSettings(rows: Record<string, ProviderRow>): {
  llm: Record<string, ProviderData>;
  stt: Record<string, ProviderData>;
} {
  const result = {
    llm: {} as Record<string, ProviderData>,
    stt: {} as Record<string, ProviderData>,
  };

  for (const [rowId, row] of Object.entries(rows)) {
    const { type, base_url, api_key } = row;
    if (type === "llm" || type === "stt") {
      const providerId = rowId.startsWith(`${type}:`)
        ? rowId.slice(type.length + 1)
        : rowId;
      result[type][providerId] = { base_url, api_key };
    }
  }

  return result;
}

export function settingsToContent(data: unknown): Content<Schemas> {
  const aiProviders = settingsToProviderRows(data);
  const values = settingsToStoreValues(data);
  return [{ ai_providers: aiProviders }, values] as Content<Schemas>;
}

export function storeToSettings(store: Store): Record<string, unknown> {
  const rows = store.getTable("ai_providers") ?? {};
  const providers = providerRowsToSettings(rows as Record<string, ProviderRow>);

  const storeValues = store.getValues();
  const settings = storeValuesToSettings(
    storeValues as Record<string, unknown>,
  );

  (settings as Record<string, Record<string, unknown>>).ai = {
    ...(settings.ai as Record<string, unknown>),
    ...providers,
  };

  return settings;
}
