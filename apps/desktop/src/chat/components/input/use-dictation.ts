import { useLingui } from "@lingui/react/macro";
import { useCallback, useRef, useState } from "react";

import type { ChatEditorHandle } from "@anlg/editor/chat";
import { commands as dictationCommands } from "@anlg/plugin-dictation";
import { commands as transcriptionCommands } from "@anlg/plugin-transcription";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useRunBatch } from "~/stt/useRunBatch";

type DictationPhase = "idle" | "starting" | "recording" | "transcribing";
const MAX_DICTATION_SECONDS = 5 * 60;

export function useDictation({
  editorRef,
  disabled,
}: {
  editorRef: React.RefObject<ChatEditorHandle | null>;
  disabled?: boolean;
}) {
  const { t } = useLingui();
  const transcriptionSessionId = useRef(
    `chat-dictation-${crypto.randomUUID()}`,
  ).current;
  const runBatch = useRunBatch(transcriptionSessionId);
  const microphoneDevice = useConfigValue("microphone_device");
  const [phase, setPhaseState] = useState<DictationPhase>("idle");
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const phaseRef = useRef<DictationPhase>("idle");
  const startedAtRef = useRef(0);
  const elapsedTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const stopRef = useRef<() => Promise<void>>(async () => {});
  const mountedRef = useRef(true);

  const setPhase = useCallback((nextPhase: DictationPhase) => {
    phaseRef.current = nextPhase;
    setPhaseState(nextPhase);
  }, []);

  const stopElapsedTimer = useCallback(() => {
    if (elapsedTimerRef.current) {
      clearInterval(elapsedTimerRef.current);
      elapsedTimerRef.current = null;
    }
  }, []);

  const start = useCallback(async () => {
    if (disabled || phaseRef.current !== "idle") {
      return;
    }

    setPhase("starting");
    try {
      const captureState = await transcriptionCommands.getCaptureState();
      if (!mountedRef.current) {
        return;
      }
      if (captureState.status === "error") {
        throw new Error(captureState.error);
      }
      if (captureState.data !== "inactive") {
        sonnerToast.warning(
          t`Voice input is unavailable while Anarlog is recording a meeting.`,
        );
        setPhase("idle");
        return;
      }

      const result = await dictationCommands.startRecording(
        microphoneDevice || null,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
      if (!mountedRef.current) {
        await cancelActiveRecording();
        return;
      }

      startedAtRef.current = Date.now();
      setElapsedSeconds(0);
      setPhase("recording");
      elapsedTimerRef.current = setInterval(() => {
        const elapsedSeconds = Math.floor(
          (Date.now() - startedAtRef.current) / 1_000,
        );
        setElapsedSeconds(Math.min(elapsedSeconds, MAX_DICTATION_SECONDS));
        if (elapsedSeconds >= MAX_DICTATION_SECONDS) {
          stopElapsedTimer();
          void stopRef.current();
        }
      }, 250);
    } catch (error) {
      await cancelActiveRecording();
      if (!mountedRef.current) {
        return;
      }
      setPhase("idle");
      sonnerToast.error(t`Could not start voice input`, {
        description: t`Check microphone permission and the selected input device, then try again.`,
      });
      console.error("[chat-dictation] failed to start recording", error);
    }
  }, [disabled, microphoneDevice, setPhase, stopElapsedTimer, t]);

  const stop = useCallback(async () => {
    if (phaseRef.current !== "recording") {
      return;
    }

    stopElapsedTimer();
    setPhase("transcribing");
    let recordedPath: string | null = null;

    try {
      const result = await dictationCommands.stopRecording();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      recordedPath = result.data.filePath;

      let transcript = "";
      await runBatch(recordedPath, {
        deferAudioFinalization: true,
        handlePersist: (words) => {
          transcript = words
            .slice()
            .sort((left, right) => left.start_ms - right.start_ms)
            .map((word) => word.text)
            .join("")
            .replace(/\s+/gu, " ")
            .trim();
        },
        keywords: [],
        notifyOnCompletion: false,
        numSpeakers: 1,
      });

      if (!transcript) {
        throw new Error("No speech was detected in the audio.");
      }
      editorRef.current?.insertText(transcript);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (/no speech|empty transcript/iu.test(message)) {
        sonnerToast.warning(t`No speech detected`, {
          description: t`Try speaking a little closer to the microphone.`,
        });
      } else {
        sonnerToast.error(t`Could not transcribe voice input`, {
          description: message,
        });
      }
      console.error("[chat-dictation] failed to transcribe recording", error);
    } finally {
      if (recordedPath) {
        await discardTemporaryRecording(recordedPath);
      }
      if (mountedRef.current) {
        setPhase("idle");
        editorRef.current?.focus();
      }
    }
  }, [editorRef, runBatch, setPhase, stopElapsedTimer, t]);
  stopRef.current = stop;

  useMountEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      stopElapsedTimer();
      if (phaseRef.current === "starting" || phaseRef.current === "recording") {
        void cancelActiveRecording();
      }
    };
  });

  return {
    elapsedSeconds,
    phase,
    start,
    stop,
  };
}

async function cancelActiveRecording() {
  try {
    const result = await dictationCommands.cancelRecording();
    if (result.status === "error") {
      throw new Error(result.error);
    }
  } catch (error) {
    console.error("[chat-dictation] failed to cancel recording", error);
  }
}

async function discardTemporaryRecording(filePath: string) {
  try {
    const result = await dictationCommands.discardRecording(filePath);
    if (result.status === "error") {
      throw new Error(result.error);
    }
  } catch (error) {
    console.error(
      "[chat-dictation] failed to discard temporary recording",
      error,
    );
  }
}
