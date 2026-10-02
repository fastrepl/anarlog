import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";

import { resolveSummaryLanguage } from "~/services/enhancer/summary-language";
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
  const language = await resolveSummaryLanguage(settingsValues, [enhancedNote]);
  return {
    language,
    enhancedNote,
    dictionaryTerms: parseDictionaryTermsJson(
      settingsValues.personalization_dictionary_terms,
    ),
  };
}
