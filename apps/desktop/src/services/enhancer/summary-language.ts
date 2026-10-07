import { commands as templateCommands } from "@anlg/plugin-template";

import { getAdditionalSpokenLanguages } from "~/settings/general/language";
import type { SettingValues } from "~/settings/schema";

export async function resolveSummaryLanguage(
  settingsValues: SettingValues,
  texts: string[],
): Promise<string | null> {
  const mainLanguage = settingsValues.ai_language;
  if (typeof mainLanguage !== "string" || mainLanguage.length === 0) {
    return null;
  }

  const additionalLanguages = getAdditionalSpokenLanguages(
    mainLanguage,
    parseStringArray(settingsValues.spoken_languages ?? "[]"),
  );
  if (
    settingsValues.summary_use_main_language === true ||
    additionalLanguages.length === 0
  ) {
    return mainLanguage;
  }

  try {
    const result = await templateCommands.dominantLanguage({
      texts,
      candidates: [mainLanguage, ...additionalLanguages],
    });
    return result.status === "ok" && result.data ? result.data : mainLanguage;
  } catch {
    return mainLanguage;
  }
}

function parseStringArray(value: string): string[] {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((entry): entry is string => typeof entry === "string")
      : [];
  } catch {
    return [];
  }
}
