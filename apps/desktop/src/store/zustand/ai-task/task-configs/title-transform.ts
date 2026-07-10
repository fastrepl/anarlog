import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";

import type { SettingValues } from "~/settings/schema";
import type { Store as MainStore } from "~/store/tinybase/store/main";
import { collectEnhancedNotesContent } from "~/store/tinybase/store/utils";

export const titleTransform: Pick<TaskConfig<"title">, "transformArgs"> = {
  transformArgs,
};

async function transformArgs(
  args: TaskArgsMap["title"],
  store: MainStore,
  settingsValues: SettingValues,
): Promise<TaskArgsMapTransformed["title"]> {
  const enhancedNote =
    args.enhancedNote ?? collectEnhancedNotesContent(store, args.sessionId);
  const language = getLanguage(settingsValues);
  return { language, enhancedNote };
}

function getLanguage(settingsValues: SettingValues): string | null {
  const value = settingsValues.ai_language;
  return typeof value === "string" && value.length > 0 ? value : null;
}
