import type { QueryClient } from "@tanstack/react-query";
import { AppState } from "react-native";

import {
  getMobileCaptureActive,
  subscribeMobileCapture,
} from "@/audio/capture-lifecycle";
import { readPreferences } from "@/settings/preferences";
import { readProviderConfig } from "@/settings/providers";

import { retryLocalTranscriptions } from "./transcribe";

export function startLocalTranscriptionRecovery(
  accountUserId: string | null,
  queryClient: QueryClient,
) {
  let active = true;
  let running = false;
  let requested = false;
  const canContinue = async () => {
    const config = await readProviderConfig(accountUserId, "stt");
    return (
      active &&
      AppState.currentState === "active" &&
      config.provider === "google_on_device" &&
      !getMobileCaptureActive()
    );
  };
  const retry = async () => {
    requested = true;
    if (running) return;
    running = true;
    try {
      do {
        requested = false;
        await retryLocalTranscriptions(accountUserId, canContinue);
      } while (active && requested);
    } finally {
      running = false;
    }
  };
  const unsubscribeCapture = subscribeMobileCapture(() => {
    if (!getMobileCaptureActive()) void retry().catch(() => {});
  });
  const subscription = AppState.addEventListener("change", (state) => {
    if (state === "active") void retry().catch(() => {});
  });
  const retryReadyModel = async (language: unknown, model: unknown) => {
    const [config, preferences] = await Promise.all([
      readProviderConfig(accountUserId, "stt"),
      readPreferences(),
    ]);
    if (
      active &&
      config.provider === "google_on_device" &&
      config.model === model &&
      (preferences.spoken_languages[0] ?? preferences.ai_language) === language
    ) {
      await retry();
    }
  };
  const unsubscribeModels = queryClient.getQueryCache().subscribe((event) => {
    if (
      event.type === "updated" &&
      event.action.type === "success" &&
      event.query.queryKey[0] === "local-speech-model" &&
      (event.query.state.data as { status?: string } | undefined)?.status ===
        "available"
    ) {
      void retryReadyModel(
        event.query.queryKey[1],
        event.query.queryKey[2],
      ).catch(() => {});
    }
  });
  void retry().catch(() => {});
  return () => {
    active = false;
    subscription.remove();
    unsubscribeCapture();
    unsubscribeModels();
  };
}
