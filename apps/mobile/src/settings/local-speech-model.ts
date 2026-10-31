import { queryOptions } from "@tanstack/react-query";

import type { Preferences } from "./preferences-model";
import { readLocalSpeechModel } from "./providers";

export function localSpeechModelOptions(
  preferences: Preferences,
  model = "mlkit-advanced",
) {
  const language = preferences.spoken_languages[0] ?? preferences.ai_language;
  return queryOptions({
    queryKey: ["local-speech-model", language, model],
    queryFn: () => readLocalSpeechModel(language, model),
    retry: false,
    staleTime: 0,
  });
}
