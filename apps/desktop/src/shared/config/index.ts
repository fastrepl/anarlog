import * as settings from "~/store/tinybase/store/settings";
import type { SettingsValueKey } from "~/store/tinybase/store/settings";

type JsonParsedKeys = "spoken_languages" | "ignored_platforms";

type ConfigValueType<K extends SettingsValueKey> = K extends JsonParsedKeys
  ? string[]
  : (typeof settings.SETTINGS_MAPPING.values)[K] extends { default: infer D }
    ? D
    : undefined;

function tryParseJSON<T>(value: any, fallback: T): T {
  if (typeof value !== "string") {
    return value;
  }
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
}

export function useConfigValue<K extends SettingsValueKey>(
  key: K,
): ConfigValueType<K> {
  const storedValue = settings.UI.useValue(key, settings.STORE_ID);
  const mapping = settings.SETTINGS_MAPPING.values[key];
  const defaultValue = "default" in mapping ? mapping.default : undefined;

  if (storedValue !== undefined) {
    if (key === "ignored_platforms" || key === "spoken_languages") {
      return tryParseJSON(
        storedValue,
        JSON.parse(defaultValue as string),
      ) as ConfigValueType<K>;
    }
    return storedValue as ConfigValueType<K>;
  }

  return defaultValue as ConfigValueType<K>;
}

export function useConfigValues<K extends SettingsValueKey>(
  keys: readonly K[],
): { [P in K]: ConfigValueType<P> } {
  const allValues = settings.UI.useValues(settings.STORE_ID);

  const result = {} as { [P in K]: ConfigValueType<P> };

  for (const key of keys) {
    const storedValue = allValues[key];
    const mapping = settings.SETTINGS_MAPPING.values[key];
    const defaultValue = "default" in mapping ? mapping.default : undefined;

    if (storedValue !== undefined) {
      if (key === "ignored_platforms" || key === "spoken_languages") {
        result[key] = tryParseJSON(
          storedValue,
          defaultValue,
        ) as ConfigValueType<K>;
      } else {
        result[key] = storedValue as ConfigValueType<K>;
      }
    } else {
      result[key] = defaultValue as ConfigValueType<K>;
    }
  }

  return result;
}
