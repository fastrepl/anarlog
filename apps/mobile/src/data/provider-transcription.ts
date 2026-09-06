import type { File } from "expo-file-system";
import { fetch } from "expo/fetch";
import { Platform } from "react-native";

import { readBoundedTranscriptionResponse } from "@/data/transcription-response";
import type { Preferences } from "@/settings/preferences-model";
import type { ProviderConfig } from "@/settings/providers-model";

export async function requestProviderTranscription(
  file: File,
  contentType: string,
  provider: ProviderConfig & { apiKey: string },
  preferences: Preferences,
  signal: AbortSignal,
) {
  if (Platform.OS !== "web" && provider.provider !== "custom") {
    const { transcribeProviderAudio, ProviderTranscriptionError } =
      await import("@anlg/mobile-bridge");
    try {
      return JSON.parse(
        await transcribeProviderAudio(
          JSON.stringify({
            provider: provider.provider,
            base_url: provider.baseUrl,
            api_key: provider.apiKey,
            file_uri: file.uri,
            params: {
              model: provider.model,
              languages: preferences.spoken_languages.length
                ? preferences.spoken_languages
                : [preferences.ai_language],
              keywords: preferences.personalization_dictionary_terms,
            },
          }),
          { signal },
        ),
      ) as { status: number; body: string };
    } catch (error) {
      if (error && typeof error === "object") {
        if (ProviderTranscriptionError.AudioTooLarge.instanceOf(error))
          throw Object.assign(
            new Error(
              `This provider accepts recordings up to ${error.inner.maxMegabytes} MB.`,
            ),
            { code: "audio_too_large", stage: "load_audio" },
          );
        if (ProviderTranscriptionError.AudioMissing.instanceOf(error))
          throw Object.assign(new Error("This recording could not be read."), {
            code: "audio_missing",
            stage: "load_audio",
          });
        if (ProviderTranscriptionError.ResponseTooLarge.instanceOf(error))
          throw Object.assign(
            new Error("The transcription response is too large."),
            { code: "stt_response_too_large", stage: "response" },
          );
        if (ProviderTranscriptionError.InvalidSettings.instanceOf(error))
          throw new Error("Check the transcription provider settings.");
        if (ProviderTranscriptionError.TimedOut.instanceOf(error))
          throw new Error(
            "The transcription provider took too long. Please try again.",
          );
        if (ProviderTranscriptionError.RequestFailed.instanceOf(error))
          throw new Error("The transcription request could not be completed.");
      }
      throw error;
    }
  }
  if (!["custom", "openai", "groq", "deepgram"].includes(provider.provider))
    throw new Error(
      "Use the iOS or Android app for this transcription provider.",
    );
  const deepgram = provider.provider === "deepgram";
  if (!deepgram && file.size > 25 * 1024 * 1024) {
    throw Object.assign(
      new Error(
        "This provider supports recordings up to 25 MB. Choose Anarlog Pro or Deepgram for longer recordings.",
      ),
      { code: "audio_too_large", stage: "load_audio" },
    );
  }
  const url = new URL(
    `${provider.baseUrl}/${deepgram ? "listen" : "audio/transcriptions"}`,
  );
  let body: File | FormData = file;
  const headers: Record<string, string> = {
    Authorization: `${deepgram ? "Token" : "Bearer"} ${provider.apiKey}`,
  };
  if (deepgram) {
    headers["Content-Type"] = contentType;
    url.searchParams.set("model", provider.model);
    url.searchParams.set("smart_format", "true");
    url.searchParams.set("diarize", "true");
    url.searchParams.set(
      "language",
      preferences.spoken_languages.length ? "multi" : preferences.ai_language,
    );
    for (const term of preferences.personalization_dictionary_terms)
      url.searchParams.append(
        provider.model.startsWith("nova-3") ? "keyterm" : "keywords",
        term,
      );
  } else {
    const form = new FormData();
    form.append("file", file, file.name);
    form.append("model", provider.model);
    form.append("language", preferences.ai_language.split("-")[0]!);
    form.append("response_format", "json");
    if (preferences.personalization_dictionary_terms.length)
      form.append(
        "prompt",
        preferences.personalization_dictionary_terms.join(", "),
      );
    body = form;
  }
  const response = await fetch(url.toString(), {
    method: "POST",
    headers,
    body,
    signal,
    redirect: "error",
  });
  return {
    status: response.status,
    body: await readBoundedTranscriptionResponse(response),
  };
}
