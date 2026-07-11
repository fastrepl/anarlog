import {
  useStoredSettingValues,
  type StoredSettingValues,
} from "~/settings/queries";
import {
  SETTING_DEFINITIONS,
  type SettingKey,
  type SettingValue,
} from "~/settings/schema";

type JsonParsedKeys =
  | "spoken_languages"
  | "personalization_dictionary_terms"
  | "ignored_platforms"
  | "included_platforms";

type ConfigValueType<K extends SettingKey> = K extends JsonParsedKeys
  ? string[]
  : K extends keyof typeof SETTING_DEFINITIONS
    ? "default" extends keyof (typeof SETTING_DEFINITIONS)[K]
      ? SettingValue<K>
      : SettingValue<K> | undefined
    : never;

const JSON_PARSED_KEYS = new Set<SettingKey>([
  "spoken_languages",
  "personalization_dictionary_terms",
  "ignored_platforms",
  "included_platforms",
]);

export function useConfigValue<K extends SettingKey>(
  key: K,
): ConfigValueType<K> {
  return resolveConfigValue(key, useStoredSettingValues());
}

export function useConfigValues<K extends SettingKey>(
  keys: readonly K[],
): { [P in K]: ConfigValueType<P> } {
  return resolveConfigValues(keys, useStoredSettingValues());
}

export function resolveConfigValues<K extends SettingKey>(
  keys: readonly K[],
  stored: StoredSettingValues,
): { [P in K]: ConfigValueType<P> } {
  const result = {} as { [P in K]: ConfigValueType<P> };
  for (const key of keys) result[key] = resolveConfigValue(key, stored);
  return result;
}

export function resolveConfigValue<K extends SettingKey>(
  key: K,
  { values, hasValues }: StoredSettingValues,
): ConfigValueType<K> {
  const definition = SETTING_DEFINITIONS[key];
  const defaultValue = "default" in definition ? definition.default : undefined;

  if (
    key === "audio_retention" &&
    values.save_recordings === false &&
    !hasValues.has("audio_retention")
  ) {
    return "none" as ConfigValueType<K>;
  }

  const value = hasValues.has(key) ? values[key] : defaultValue;
  if (JSON_PARSED_KEYS.has(key)) {
    return parseStringArray(
      value,
      parseStringArray(defaultValue, []),
    ) as ConfigValueType<K>;
  }

  return value as ConfigValueType<K>;
}

function parseStringArray(value: unknown, fallback: string[]): string[] {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === "string");
  }
  if (typeof value !== "string") return fallback;
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((entry): entry is string => typeof entry === "string")
      : fallback;
  } catch {
    return fallback;
  }
}
