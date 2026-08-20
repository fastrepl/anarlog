import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";

import { loadSessionContentSnapshot } from "~/session/content-queries";
import type { SettingValues } from "~/settings/schema";
import { parseDictionaryTermsJson } from "~/stt/keywords";

export const titleTransform: Pick<TaskConfig<"title">, "transformArgs"> = {
  transformArgs,
};

async function transformArgs(
  args: TaskArgsMap["title"],
  settingsValues: SettingValues,
): Promise<TaskArgsMapTransformed["title"]> {
  const snapshot = args.enhancedNote
    ? null
    : await loadSessionContentSnapshot(args.sessionId);
  if (!args.enhancedNote && !snapshot) {
    throw new Error(`Session ${args.sessionId} no longer exists`);
  }

  const enhancedNote =
    args.enhancedNote ??
    snapshot?.enhancedNotes
      .map((note) => note.markdown)
      .filter(Boolean)
      .join("\n\n") ??
    "";
  const language = getLanguage(settingsValues);
  return {
    language,
    enhancedNote,
    dictionaryTerms: parseDictionaryTermsJson(
      settingsValues.personalization_dictionary_terms,
    ),
  };
}

function getLanguage(settingsValues: SettingValues): string | null {
  const value = settingsValues.ai_language;
  return typeof value === "string" && value.length > 0 ? value : null;
}
