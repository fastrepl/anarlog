import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";

import type { Store as MainStore } from "~/store/tinybase/store/main";
import type { Store as SettingsStore } from "~/store/tinybase/store/settings";
import { collectEnhancedNotesContent } from "~/store/tinybase/store/utils";

export const titleTransform: Pick<TaskConfig<"title">, "transformArgs"> = {
  transformArgs,
};

async function transformArgs(
  args: TaskArgsMap["title"],
  store: MainStore,
  settingsStore: SettingsStore,
): Promise<TaskArgsMapTransformed["title"]> {
  const enhancedNote = collectEnhancedNotesContent(store, args.sessionId);
  const language = getLanguage(settingsStore);
  return { language, enhancedNote };
}

function getLanguage(settingsStore: SettingsStore): string | null {
  const value = settingsStore.getValue("ai_language");
  return typeof value === "string" && value.length > 0 ? value : null;
}
