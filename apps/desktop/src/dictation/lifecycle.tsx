import { platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";
import { create } from "zustand";

import { commands as dictation } from "@anlg/plugin-dictation";
import { commands as permissions } from "@anlg/plugin-permissions";
import { commands as shortcuts, events } from "@anlg/plugin-shortcut";
import { commands as transcription } from "@anlg/plugin-transcription";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { DictationController, type DictationPhase } from "./controller";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useSettingsReady } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useListener } from "~/stt/contexts";
import { useRunBatch } from "~/stt/useRunBatch";

export const useDictationStatus = create<{
  phase: DictationPhase;
  error: string | null;
  lastTranscript: string;
  ready: boolean;
  retry: number;
  cancel: (() => void) | null;
}>(() => ({
  phase: "idle",
  error: null,
  lastTranscript: "",
  ready: false,
  retry: 0,
  cancel: null,
}));

let lifecycle: Promise<void> = Promise.resolve();

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
  const current = useRef({ runBatch, microphone });
  current.current = { runBatch, microphone };

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
      sonnerToast.error(message);
    };
    const controller = new DictationController({
      handsFree,
      start: async () => {
        abort = new AbortController();
        useDictationStatus.setState({ error: null });
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
        unwrap(await dictation.setPhase("recording"));
        unwrap(await dictation.show());
        abort.signal.throwIfAborted();
        unwrap(
          await dictation.startRecording(
            current.current.microphone || null,
            id,
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
        if (!disposed) useDictationStatus.setState({ phase });
        presentation = presentation
          .then(async () => {
            try {
              unwrap(await shortcuts.setActive(phase !== "idle" && !disposed));
            } catch (error) {
              onError(error);
            }
            if (phase === "idle" || disposed) {
              unwrap(await dictation.hide());
            } else {
              unwrap(
                await dictation.setPhase(
                  phase === "transcribing" ? "processing" : "recording",
                ),
              );
              unwrap(await dictation.show());
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
          useDictationStatus.setState({ ready: true, error: null, cancel });
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
      });
      lifecycle = lifecycle
        .then(async () => {
          unlisten?.();
          await controller.cancel();
          await presentation;
          unwrap(await shortcuts.configure(null));
          unwrap(await dictation.hide());
        })
        .catch(() => {});
    };
  });
  return null;
}
