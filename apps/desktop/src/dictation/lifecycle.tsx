import { Channel } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";

import {
  commands as dictation,
  type RecordingUpdate,
} from "@anlg/plugin-dictation";
import { commands as permissions } from "@anlg/plugin-permissions";
import { commands as shortcuts, events } from "@anlg/plugin-shortcut";
import { commands as transcription } from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";

import { DictationController } from "./controller";
import { waitForDictationPanel } from "./panel";
import { useDictationStatus } from "./state";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useSettingsReady } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useListener } from "~/stt/contexts";
import { useRunBatch } from "~/stt/useRunBatch";
import { useSTTConnection } from "~/stt/useSTTConnection";
export { useDictationStatus } from "./state";

let lifecycle: Promise<void> = Promise.resolve();

export function waitForDictationCleanup() {
  return lifecycle;
}

export function DictationLifecycle() {
  const { session } = useAuth();
  const { isPro, isReady } = useBillingAccess();
  const settingsReady = useSettingsReady();
  const enabled = useConfigValue("dictation_enabled");
  const shortcut = useConfigValue("dictation_shortcut");
  const handsFree = useConfigValue("dictation_hands_free");
  const retry = useDictationStatus((state) => state.retry);
  const meetingActive = useListener(
    (state) => state.live.status !== "inactive" || state.live.loading,
  );

  if (!session || !isReady || !isPro || !settingsReady || !enabled) return null;
  return (
    <TranscriptRetention key={session.user.id}>
      {!meetingActive && (
        <ActiveDictation
          key={`${shortcut}:${handsFree}:${retry}`}
          shortcut={shortcut}
          handsFree={handsFree}
        />
      )}
    </TranscriptRetention>
  );
}

function TranscriptRetention({ children }: { children: React.ReactNode }) {
  useMountEffect(() => () => {
    useDictationStatus.setState({ lastTranscript: "" });
  });
  return children;
}

function unwrap<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: string },
): T {
  if (result.status === "error") throw new Error(result.error);
  return result.data;
}

function ActiveDictation({
  shortcut,
  handsFree,
}: {
  shortcut: string;
  handsFree: boolean;
}) {
  const id = useRef(`system-dictation-${crypto.randomUUID()}`).current;
  const runBatch = useRunBatch(id);
  const stopTranscription = useListener((state) => state.stopTranscription);
  const microphone = useConfigValue("microphone_device");
  const livePreview = useConfigValue("dictation_live_preview");
  const dictionary = useConfigValue("personalization_dictionary_terms");
  const languages = useConfigValue("spoken_languages");
  const { conn, isCloudModel } = useSTTConnection();
  const auth = useAuth();
  const current = useRef({
    runBatch,
    microphone,
    livePreview,
    languages,
    dictionary,
    conn,
    isCloudModel,
    auth,
  });
  current.current = {
    runBatch,
    microphone,
    livePreview,
    languages,
    dictionary,
    conn,
    isCloudModel,
    auth,
  };

  useMountEffect(() => {
    let disposed = false;
    let armed = false;
    let target = "";
    let unlisten: (() => void) | undefined;
    let abort = new AbortController();
    let presentation: Promise<void> = Promise.resolve();
    const onError = (error: unknown) => {
      if (disposed) return;
      const message = error instanceof Error ? error.message : String(error);
      useDictationStatus.setState({ error: message });
      toast.error(message);
    };
    const controller = new DictationController({
      handsFree,
      start: async () => {
        abort = new AbortController();
        const recordingAbort = abort;
        const owner = `${id}:${crypto.randomUUID()}`;
        const {
          microphone,
          livePreview,
          conn,
          isCloudModel,
          languages,
          dictionary,
          auth,
        } = current.current;
        useDictationStatus.setState({
          owner,
          error: null,
          text: "",
          partial: "",
          amplitude: 0,
          microphone: microphone || "Default microphone",
          previewEnabled: livePreview,
          previewUnavailable: livePreview && !conn,
          expanded: livePreview,
        });
        if (platform() === "macos") {
          const permission = unwrap(
            await permissions.checkPermission("microphone"),
          );
          if (permission !== "authorized")
            throw new Error(
              "Enable microphone permission in Settings > Permissions before dictating.",
            );
        }
        const capture = unwrap(await transcription.getCaptureState());
        if (capture !== "inactive")
          throw new Error(
            "Dictation is unavailable while Anarlog is recording a meeting.",
          );
        abort.signal.throwIfAborted();
        target = unwrap(await dictation.captureTarget());
        const previewSession =
          livePreview && isCloudModel
            ? await auth.getSessionForRequest().catch(() => null)
            : null;
        const apiKey = isCloudModel
          ? previewSession?.access_token
          : conn?.apiKey;
        const preview =
          livePreview && conn && (!isCloudModel || previewSession)
            ? {
                provider: conn.provider,
                baseUrl: conn.baseUrl,
                apiKey: apiKey ?? "",
                params: {
                  model: conn.model,
                  channels: 1,
                  sample_rate: 16000,
                  languages: languages || [],
                  keywords: dictionary || [],
                  num_speakers: 1,
                  min_speakers: null,
                  max_speakers: null,
                  custom_query: null,
                },
              }
            : null;
        useDictationStatus.setState({
          previewUnavailable: livePreview && !preview,
        });
        abort.signal.throwIfAborted();
        await waitForDictationPanel(owner, abort.signal);
        abort.signal.throwIfAborted();
        unwrap(
          await dictation.startSystemRecording(
            microphone || null,
            id,
            preview,
            new Channel<RecordingUpdate>((update) => {
              if (
                disposed ||
                recordingAbort.signal.aborted ||
                recordingAbort !== abort
              )
                return;
              if (
                !["starting", "recording"].includes(
                  useDictationStatus.getState().phase,
                )
              )
                return;
              if (update.type === "amplitude")
                useDictationStatus.setState({ amplitude: update.amplitude });
              else if (update.type === "transcript")
                useDictationStatus.setState({
                  text: update.text,
                  partial: update.partial,
                });
              else
                useDictationStatus.setState({
                  previewUnavailable: true,
                  partial: "",
                });
            }),
          ),
        );
      },
      stop: async () => unwrap(await dictation.stopRecording(id)).filePath,
      cancel: async () => {
        unwrap(await dictation.cancelRecording(id));
      },
      transcribe: async (path) => {
        let text = "";
        await current.current.runBatch(path, {
          signal: abort.signal,
          deferAudioFinalization: true,
          notifyOnCompletion: false,
          numSpeakers: 1,
          handlePersist: (words) => {
            text = words
              .slice()
              .sort((a, b) => a.start_ms - b.start_ms)
              .map((word) => word.text)
              .join("")
              .replace(/\s+/gu, " ")
              .trim();
          },
        });
        return text;
      },
      insert: async (text) => {
        if (!disposed) unwrap(await dictation.insertText(target, text));
      },
      discard: async (path) => {
        unwrap(await dictation.discardRecording(path));
      },
      onTranscript: (lastTranscript) => {
        if (!disposed) useDictationStatus.setState({ lastTranscript });
      },
      onError,
      onPhase: (phase) => {
        if (!disposed)
          useDictationStatus.setState({
            phase,
            ...(phase === "idle"
              ? {
                  owner: null,
                  presentedOwner: null,
                  text: "",
                  partial: "",
                  amplitude: 0,
                }
              : {}),
            ...(phase === "starting" &&
            useDictationStatus.getState().phase === "idle"
              ? { owner: null }
              : {}),
          });
        presentation = presentation
          .then(async () => {
            try {
              unwrap(await shortcuts.setActive(phase !== "idle" && !disposed));
            } catch (error) {
              onError(error);
            }
          })
          .catch(onError);
      },
    });
    const cancel = () => {
      abort.abort();
      void controller.cancel();
      void stopTranscription(id).catch(onError);
    };

    lifecycle = lifecycle
      .then(async () => {
        if (disposed) return;
        unlisten = await events.shortcutEvent.listen(({ payload }) => {
          if (disposed || !armed) return;
          if (payload.type === "pressed") controller.press();
          else if (payload.type === "released") controller.release();
          else cancel();
        });
        if (disposed) {
          unlisten();
          return;
        }
        unwrap(await shortcuts.configure(shortcut));
        armed = !disposed;
        if (!disposed)
          useDictationStatus.setState({
            ready: true,
            error: null,
            cancel,
            finish: () => controller.release(true),
          });
      })
      .catch(onError);

    return () => {
      disposed = true;
      armed = false;
      unlisten?.();
      cancel();
      useDictationStatus.setState({
        ready: false,
        phase: "idle",
        lastTranscript: useDictationStatus.getState().lastTranscript,
        cancel: null,
        finish: null,
        owner: null,
        text: "",
        partial: "",
      });
      lifecycle = lifecycle
        .then(async () => {
          unlisten?.();
          await controller.cancel();
          await presentation;
          unwrap(await shortcuts.configure(null));
        })
        .catch(() => {});
    };
  });
  return null;
}
