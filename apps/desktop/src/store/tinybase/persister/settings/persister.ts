import { createCustomPersister } from "tinybase/persisters/with-schemas";
import type { Content } from "tinybase/with-schemas";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as detectCommands } from "@hypr/plugin-detect";
import { commands } from "@hypr/plugin-settings";

import { settingsToContent, storeToSettings } from "./transform";

import { createFileListener } from "~/store/tinybase/persister/shared/listener";
import type { Schemas, Store } from "~/store/tinybase/store/settings";
import { StoreOrMergeableStore } from "~/store/tinybase/store/shared";

const SETTINGS_FILENAME = "settings.json";

async function getLanguageDefaults(): Promise<{
  ai_language?: string;
  spoken_languages?: string[];
}> {
  const result = await detectCommands.getPreferredLanguages();
  if (result.status !== "ok" || result.data.length === 0) {
    return {};
  }
  return {
    ai_language: result.data[0],
    spoken_languages: result.data,
  };
}

function applyLanguageDefaults(
  settings: Record<string, unknown>,
  defaults: { ai_language?: string; spoken_languages?: string[] },
): Record<string, unknown> {
  const language = (settings.language ?? {}) as Record<string, unknown>;

  if (language.ai_language == null && defaults.ai_language) {
    language.ai_language = defaults.ai_language;
  }
  if (language.spoken_languages == null && defaults.spoken_languages) {
    language.spoken_languages = defaults.spoken_languages;
  }

  return { ...settings, language };
}

const settingsNotifyListener = createFileListener({
  mode: "simple",
  pathMatcher: (path) => path.endsWith(SETTINGS_FILENAME),
});

export const createSettingsPersister = createPersisterBuilder({
  toStore: settingsToContent,
  fromStore: storeToSettings,
});

interface TransformUtils<T> {
  toStore: (data: T) => Content<Schemas>;
  fromStore: (store: Store) => T;
}

function createPersisterBuilder<T>(transform: TransformUtils<T>) {
  return (store: Store) =>
    createCustomPersister(
      store,
      async (): Promise<Content<Schemas> | undefined> => {
        const [result, languageDefaults] = await Promise.all([
          commands.load(),
          getLanguageDefaults(),
        ]);

        if (result.status === "error") {
          console.error("[SettingsPersister] load error:", result.error);
          return undefined;
        }

        const settings = applyLanguageDefaults(
          result.data as Record<string, unknown>,
          languageDefaults,
        );

        return transform.toStore(settings as T);
      },
      async () => {
        const settings = transform.fromStore(store);
        const result = await commands.save(
          settings as Parameters<typeof commands.save>[0],
        );
        if (result.status === "error") {
          console.error("[SettingsPersister] save error:", result.error);
        }

        const s = settings as Record<string, Record<string, unknown>>;
        void analyticsCommands.setProperties({
          set: {
            spoken_languages: (s.language?.spoken_languages as string[]) ?? [],
            current_stt_provider:
              (s.ai?.current_stt_provider as string) ?? null,
            current_stt_model: (s.ai?.current_stt_model as string) ?? null,
            current_llm_provider:
              (s.ai?.current_llm_provider as string) ?? null,
            current_llm_model: (s.ai?.current_llm_model as string) ?? null,
          },
        });
      },
      (listener) => settingsNotifyListener.addListener(listener),
      (handle) => settingsNotifyListener.delListener(handle),
      (error) => console.error("[SettingsPersister]:", error),
      StoreOrMergeableStore,
    );
}
