import { useLingui } from "@lingui/react/macro";
import { useCallback, useRef, useState } from "react";

import type { ChatEditorHandle } from "@anlg/editor/chat";
import { commands as dictationCommands } from "@anlg/plugin-dictation";
import { commands as transcriptionCommands } from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { useConfigValue } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";
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
  const generationRef = useRef(0);
  const startInFlightRef = useRef(false);
  const cancellationRef = useRef<Promise<void> | null>(null);

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

  const cancelRecording = useCallback(() => {
    const cancellation = (cancellationRef.current ?? Promise.resolve()).then(
      () => cancelActiveRecording(transcriptionSessionId),
    );
    cancellationRef.current = cancellation;
    void cancellation.finally(() => {
      if (cancellationRef.current === cancellation) {
        cancellationRef.current = null;
      }
    });
    return cancellation;
  }, [transcriptionSessionId]);

  const start = useCallback(async () => {
    if (
      disabled ||
      !mountedRef.current ||
      startInFlightRef.current ||
      phaseRef.current !== "idle"
    ) {
      return;
    }

    setPhase("starting");
    startInFlightRef.current = true;
    const generation = generationRef.current;
    try {
      await cancellationRef.current;
      if (!mountedRef.current || generation !== generationRef.current) {
        return;
      }
      const captureState = await transcriptionCommands.getCaptureState();
      if (!mountedRef.current || generation !== generationRef.current) {
        return;
      }
      if (captureState.status === "error") {
        throw new Error(captureState.error);
      }
      if (captureState.data !== "inactive") {
        toast.warning(
          t`Voice input is unavailable while Anarlog is recording a meeting.`,
        );
        setPhase("idle");
        return;
      }

      const result = await dictationCommands.startRecording(
        microphoneDevice || null,
        transcriptionSessionId,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
      if (!mountedRef.current || generation !== generationRef.current) {
        await cancelRecording();
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
      await cancelRecording();
      if (!mountedRef.current || generation !== generationRef.current) {
        return;
      }
      setPhase("idle");
      toast.error(t`Could not start voice input`, {
        description: t`Check microphone permission and the selected input device, then try again.`,
      });
      console.error("[chat-dictation] failed to start recording", error);
    } finally {
      startInFlightRef.current = false;
    }
  }, [
    cancelRecording,
    disabled,
    microphoneDevice,
    setPhase,
    stopElapsedTimer,
    t,
    transcriptionSessionId,
  ]);

  const stop = useCallback(async () => {
    if (phaseRef.current !== "recording") {
      return;
    }

    stopElapsedTimer();
    setPhase("transcribing");
    const generation = generationRef.current;
    let recordedPath: string | null = null;

    try {
      const result = await dictationCommands.stopRecording(
        transcriptionSessionId,
      );
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
      if (mountedRef.current && generation === generationRef.current) {
        editorRef.current?.insertText(transcript);
      }
    } catch (error) {
      if (!mountedRef.current || generation !== generationRef.current) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      if (/no speech|empty transcript/iu.test(message)) {
        toast.warning(t`No speech detected`, {
          description: t`Try speaking a little closer to the microphone.`,
        });
      } else {
        toast.error(t`Could not transcribe voice input`, {
          description: message,
        });
      }
      console.error("[chat-dictation] failed to transcribe recording", error);
    } finally {
      if (recordedPath) {
        await discardTemporaryRecording(recordedPath);
      }
      setPhase("idle");
      if (mountedRef.current && generation === generationRef.current) {
        editorRef.current?.focus();
      }
    }
  }, [
    editorRef,
    runBatch,
    setPhase,
    stopElapsedTimer,
    t,
    transcriptionSessionId,
  ]);
  stopRef.current = stop;

  useMountEffect(() => {
    mountedRef.current = true;
    const unsubscribe = useTabs.subscribe((state, previous) => {
      if (state.chatMode === previous.chatMode) {
        return;
      }
      if (state.chatMode !== "FloatingClosed") {
        mountedRef.current = true;
        return;
      }

      mountedRef.current = false;
      generationRef.current += 1;
      stopElapsedTimer();
      if (phaseRef.current === "starting" || phaseRef.current === "recording") {
        void cancelRecording();
      }
      if (phaseRef.current !== "transcribing") {
        setPhase("idle");
      }
    });
    return () => {
      unsubscribe();
      mountedRef.current = false;
      generationRef.current += 1;
      stopElapsedTimer();
      if (phaseRef.current === "starting" || phaseRef.current === "recording") {
        void cancelRecording();
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

async function cancelActiveRecording(transcriptionSessionId: string) {
  try {
    const result = await dictationCommands.cancelRecording(
      transcriptionSessionId,
    );
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
