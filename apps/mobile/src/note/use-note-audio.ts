import { useMutation } from "@tanstack/react-query";
import { File, Paths } from "expo-file-system";
import { useRef } from "react";
import { Alert } from "react-native";

import { useAuth } from "@/auth/context";
import { useSessionAudio } from "@/data/audio-catalog";
import { importRecordingIntoSession } from "@/data/import-voice-memo";
import {
  restoreSessionAudioFromCloud,
  restoreSessionAudioFromPicker,
} from "@/data/restore-session-audio";
import { captureAnalytics } from "@/lib/analytics";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

export function useNoteAudio(sessionId: string, hasTranscript: boolean) {
  const auth = useAuth();
  const audio = useSessionAudio(sessionId);
  const controllerRef = useRef<AbortController | null>(null);
  const mutation = useMutation({
    mutationFn: async ({
      action,
      controller,
    }: {
      action: "choose" | "download" | "import";
      controller: AbortController;
    }) => {
      try {
        if (controller.signal.aborted) return;
        if (action === "import") {
          await importRecordingIntoSession(sessionId, auth.session?.user.id);
          return;
        }
        const recording = audio.data;
        if (!recording) return;
        if (action === "choose") {
          const result = await restoreSessionAudioFromPicker(
            sessionId,
            recording,
          );
          if (result === "restored") {
            captureAnalytics("file_uploaded", {
              entry_point: "mobile_audio_restore",
              file_type: "audio",
              content_type: recording.contentType,
              size_bytes: recording.sizeBytes,
            });
          }
        } else {
          const accessToken = auth.session?.access_token;
          if (!accessToken || !env.supabaseUrl) return;
          await restoreSessionAudioFromCloud(sessionId, recording, {
            accessToken,
            apiBaseUrl: env.apiUrl,
            supabaseUrl: env.supabaseUrl,
            signal: controller.signal,
          });
          captureAnalytics("audio_restored", {
            entry_point: "cloud_sync",
            content_type: recording.contentType,
            size_bytes: recording.sizeBytes,
          });
        }
      } catch (error) {
        if (controller.signal.aborted) return;
        captureOperationalError(error, {
          operation:
            action === "import"
              ? "voice_memo_import"
              : action === "download"
                ? "session_audio_cloud_restore"
                : "session_audio_restore",
          ...(action === "import" && { tags: { entry_point: "mobile_note" } }),
        });
        const message =
          error instanceof Error
            ? error.message
            : action === "import"
              ? "The selected recording could not be imported."
              : action === "download"
                ? "The recording could not be downloaded to this phone."
                : "The recording could not be added to this phone.";
        if (action === "import")
          Alert.alert("Couldn’t import recording", message);
        throw new Error(message);
      } finally {
        if (controllerRef.current === controller) controllerRef.current = null;
      }
    },
  });

  useMountEffect(() => () => controllerRef.current?.abort());

  const run = (action: "choose" | "download" | "import") => {
    // Mutation state updates on the next render; the controller also guards same-frame taps.
    if (controllerRef.current) return;
    if (
      action === "import" ? Boolean(audio.data) || hasTranscript : !audio.data
    )
      return;
    if (
      action === "download" &&
      (!audio.data?.cloudObjectKey ||
        !auth.session?.access_token ||
        !env.supabaseUrl)
    )
      return;
    const controller = new AbortController();
    controllerRef.current = controller;
    mutation.mutate({ action, controller });
  };
  const file = audio.data?.localRelativePath
    ? new File(
        Paths.document,
        "sessions",
        sessionId,
        audio.data.localRelativePath,
      )
    : null;
  return {
    ...audio,
    file,
    availableLocally:
      audio.data?.availableLocally === true && file?.exists === true,
    restore: {
      cloudAvailable: Boolean(
        audio.data?.cloudObjectKey &&
        auth.billing.isPro &&
        auth.session?.access_token &&
        env.supabaseUrl,
      ),
      loading: mutation.isPending && mutation.variables.action !== "import",
      errorMessage:
        mutation.variables?.action === "import"
          ? null
          : (mutation.error?.message ?? null),
      onChooseRecording: () => run("choose"),
      onDownloadRecording: () => run("download"),
    },
    importRecording: () => run("import"),
  };
}
