import {
  getRecordingPermissionsAsync,
  requestRecordingPermissionsAsync,
  setAudioModeAsync,
  useAudioStream,
  type AudioStreamBuffer,
} from "expo-audio";
import { useCallback, useRef, useState } from "react";
import { AppState } from "react-native";

import {
  beginMobileCapture,
  endMobileCapture,
} from "@/audio/capture-lifecycle";
import { pcmAmplitude } from "@/audio/pcm-wav";
import { type RecorderPhase } from "@/audio/recorder-status";
import { SessionWavWriter } from "@/audio/session-wav-writer";
import { catalogSessionAudio } from "@/data/audio-catalog";
import {
  HostedLiveTranscription,
  markSessionAudioTranscribed,
  type LiveTranscriptionStatus,
} from "@/data/live-transcription";
import { transcribeSession } from "@/data/transcribe";
import { captureAnalytics } from "@/lib/analytics";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

const STREAM_SAMPLE_RATE = 16_000;
const STREAM_CHANNELS = 1;

export type { RecorderPhase } from "@/audio/recorder-status";

export type RecorderFailure =
  | "permission_denied"
  | "start_failed"
  | "media_services_reset"
  | "native_error"
  | "save_failed";

export type StopResult = "saved" | "failed" | "noop";

export function useSessionRecorder(
  sessionId: string,
  enabled: boolean,
): {
  phase: RecorderPhase;
  failure: RecorderFailure | null;
  amplitude: number;
  durationMs: number;
  liveStatus: LiveTranscriptionStatus;
  liveTranscript: string;
  start: () => Promise<void>;
  stop: () => Promise<StopResult>;
  retry: () => Promise<StopResult>;
} {
  const [phase, setPhaseState] = useState<RecorderPhase>("idle");
  const [failure, setFailure] = useState<RecorderFailure | null>(null);
  const [amplitude, setAmplitude] = useState(0);
  const [durationMs, setDurationMs] = useState(0);
  const [liveStatus, setLiveStatus] =
    useState<LiveTranscriptionStatus>("connecting");
  const [liveTranscript, setLiveTranscript] = useState("");
  const phaseRef = useRef<RecorderPhase>("idle");
  const activeRef = useRef(true);
  const writerRef = useRef<SessionWavWriter | null>(null);
  const liveRef = useRef<HostedLiveTranscription | null>(null);
  const startGenerationRef = useRef(0);
  const startRef = useRef<Promise<void> | null>(null);
  const stopOperationRef = useRef<Promise<StopResult> | null>(null);
  const completionTrackedRef = useRef(false);
  const reportedFailureRef = useRef<string | null>(null);
  const durationRef = useRef(0);
  const captureRegisteredRef = useRef(false);

  const registerCapture = useCallback(() => {
    if (captureRegisteredRef.current) return;
    captureRegisteredRef.current = true;
    beginMobileCapture(sessionId);
  }, [sessionId]);

  const unregisterCapture = useCallback(() => {
    if (!captureRegisteredRef.current) return;
    captureRegisteredRef.current = false;
    endMobileCapture(sessionId);
  }, [sessionId]);

  const setPhase = useCallback((next: RecorderPhase) => {
    phaseRef.current = next;
    if (activeRef.current) setPhaseState(next);
  }, []);

  const reportFailure = useCallback(
    (reason: RecorderFailure, error: unknown, operation: string) => {
      if (reportedFailureRef.current !== reason) {
        reportedFailureRef.current = reason;
        captureAnalytics("recording_failed", { failure_stage: reason });
      }
      if (reason !== "permission_denied") {
        captureOperationalError(error, {
          operation,
          tags: { stage: reason },
        });
      }
      if (activeRef.current) setFailure(reason);
    },
    [],
  );

  const handleBuffer = useCallback(
    (buffer: AudioStreamBuffer) => {
      if (phaseRef.current !== "starting" && phaseRef.current !== "recording") {
        return;
      }
      try {
        writerRef.current?.append(
          buffer.data,
          buffer.sampleRate,
          buffer.channels,
        );
      } catch (error) {
        reportFailure("save_failed", error, "recording_stream_write");
        setPhase("save_error");
        try {
          streamRef.current.stop();
        } catch {}
        return;
      }
      const generation = startGenerationRef.current;
      liveRef.current ??= new HostedLiveTranscription(
        sessionId,
        buffer.sampleRate,
        buffer.channels,
        ({ status, text }) => {
          if (!activeRef.current || generation !== startGenerationRef.current) {
            return;
          }
          setLiveStatus(status);
          if (text !== "") setLiveTranscript(text);
        },
      );
      liveRef.current?.sendAudio(buffer.data);
      const frameDuration =
        buffer.data.byteLength /
        2 /
        Math.max(1, buffer.channels) /
        Math.max(1, buffer.sampleRate);
      durationRef.current = Math.max(
        durationRef.current,
        Math.round((buffer.timestamp + frameDuration) * 1_000),
      );
      if (activeRef.current) {
        setAmplitude(pcmAmplitude(buffer.data));
        setDurationMs(durationRef.current);
      }
    },
    [reportFailure, sessionId, setPhase],
  );

  const { stream } = useAudioStream({
    sampleRate: STREAM_SAMPLE_RATE,
    channels: STREAM_CHANNELS,
    encoding: "int16",
    onBuffer: handleBuffer,
  });
  const streamRef = useRef(stream);
  streamRef.current = stream;

  const performStart = useCallback(async () => {
    const generation = ++startGenerationRef.current;
    const isCurrent = () =>
      activeRef.current && generation === startGenerationRef.current;
    completionTrackedRef.current = false;
    durationRef.current = 0;
    setDurationMs(0);
    setAmplitude(0);
    setLiveStatus("connecting");
    setLiveTranscript("");
    setFailure(null);
    reportedFailureRef.current = null;
    setPhase("starting");
    try {
      let permission = await getRecordingPermissionsAsync();
      if (!permission.granted) {
        captureAnalytics("permission_requested", {
          permission: "microphone",
          entry_point: "start_listening",
          action: "request",
        });
        permission = await requestRecordingPermissionsAsync();
        captureAnalytics("permission_resolved", {
          permission: "microphone",
          entry_point: "start_listening",
          status: permission.granted ? "authorized" : "denied",
        });
      }
      if (!isCurrent()) return;
      if (!permission.granted) {
        reportFailure(
          "permission_denied",
          new Error("Microphone permission denied"),
          "recording_permission",
        );
        captureAnalytics("session_start_failed", {
          failure_stage: "microphone_permission",
        });
        setPhase("unavailable");
        return;
      }

      await setAudioModeAsync({
        allowsRecording: true,
        playsInSilentMode: true,
        allowsBackgroundRecording: true,
        interruptionMode: "doNotMix",
      });
      if (!isCurrent()) return;
      writerRef.current = new SessionWavWriter(sessionId);
      registerCapture();
      await stream.start();
      if (!isCurrent()) {
        phaseRef.current = "recording";
        return;
      }
      captureAnalytics("recording_started", {
        entry_point: "mobile_recorder",
        transcription_mode: "live_with_batch_fallback",
      });
      captureAnalytics("session_started", {
        entry_point: "mobile_recorder",
        transcription_mode: "live_with_batch_fallback",
      });
      setPhase("recording");
    } catch (error) {
      try {
        stream.stop();
      } catch {}
      try {
        writerRef.current?.closeAndDiscardEmpty();
      } catch (cleanupError) {
        captureOperationalError(cleanupError, {
          operation: "recording_start_cleanup",
        });
      }
      writerRef.current = null;
      void liveRef.current?.stop();
      liveRef.current = null;
      unregisterCapture();
      reportFailure("start_failed", error, "recording_start");
      captureAnalytics("session_start_failed", {
        failure_stage: "capture_start",
      });
      setPhase("error");
    }
  }, [
    registerCapture,
    reportFailure,
    sessionId,
    setPhase,
    stream,
    unregisterCapture,
  ]);

  const start = useCallback((): Promise<void> => {
    const currentOperation = startRef.current;
    if (currentOperation) return currentOperation;
    if (
      !["idle", "unavailable", "interrupted", "error"].includes(
        phaseRef.current,
      )
    ) {
      return Promise.resolve();
    }
    const operation = performStart();
    startRef.current = operation;
    const clearOperation = () => {
      if (startRef.current === operation) startRef.current = null;
    };
    void operation.then(clearOperation, clearOperation);
    return operation;
  }, [performStart]);

  useMountEffect(() => {
    activeRef.current = true;
    if (!enabled) return;
    void start();
  });

  useMountEffect(() => {
    let previousState = AppState.currentState;
    const subscription = AppState.addEventListener("change", (nextState) => {
      const returnedFromSettings =
        previousState !== "active" &&
        nextState === "active" &&
        phaseRef.current === "unavailable";
      previousState = nextState;
      if (returnedFromSettings) void start();
    });
    return () => subscription.remove();
  });

  const performStop = async (): Promise<StopResult> => {
    await startRef.current?.catch(() => {});
    if (
      !["recording", "save_error", "error", "interrupted"].includes(
        phaseRef.current,
      )
    ) {
      return "noop";
    }
    if (!writerRef.current) return "noop";
    setPhase("saving");
    try {
      streamRef.current.stop();
      const writer = writerRef.current;
      if (!writer) throw new Error("Recording file is unavailable");
      const live = liveRef.current;
      writer.finalize();
      await catalogSessionAudio(sessionId, {
        filename: "audio.wav",
        contentType: "audio/wav",
      });
      const liveComplete = await (live?.stop() ?? Promise.resolve(false));
      let transcriptionMode = liveComplete ? "live" : "batch_fallback";
      if (liveComplete) {
        await markSessionAudioTranscribed(sessionId).catch((error) => {
          transcriptionMode = "batch_fallback";
          captureOperationalError(error, {
            operation: "transcription_live_mark_complete",
            tags: { mode: "live" },
          });
          void transcribeSession(sessionId);
        });
      } else {
        void transcribeSession(sessionId);
      }
      if (!completionTrackedRef.current) {
        completionTrackedRef.current = true;
        const properties = {
          duration_seconds: Math.round(durationRef.current / 1_000),
          completion_reason: "user_stopped",
          transcription_requested: true,
          transcription_mode: transcriptionMode,
        };
        captureAnalytics("recording_completed", properties);
        captureAnalytics("session_completed", properties);
      }
      writerRef.current = null;
      liveRef.current = null;
      unregisterCapture();
      setFailure(null);
      setPhase("saved");
      return "saved";
    } catch (error) {
      reportFailure("save_failed", error, "recording_save");
      setPhase("save_error");
      return "failed";
    }
  };

  const stop = (): Promise<StopResult> => {
    const currentOperation = stopOperationRef.current;
    if (currentOperation) return currentOperation;
    const operation = performStop();
    stopOperationRef.current = operation;
    const clearOperation = () => {
      if (stopOperationRef.current === operation) {
        stopOperationRef.current = null;
      }
    };
    void operation.then(clearOperation, clearOperation);
    return operation;
  };

  const retry = async (): Promise<StopResult> => {
    if (
      writerRef.current &&
      ["save_error", "interrupted", "error"].includes(phaseRef.current)
    ) {
      return stop();
    }
    if (["unavailable", "interrupted", "error"].includes(phaseRef.current)) {
      await start();
    }
    return "noop";
  };

  const stopRef = useRef(stop);
  stopRef.current = stop;
  useMountEffect(() => () => {
    activeRef.current = false;
    startGenerationRef.current += 1;
    void stopRef.current().then(unregisterCapture, unregisterCapture);
  });

  return {
    phase,
    failure,
    amplitude,
    durationMs,
    liveStatus,
    liveTranscript,
    start,
    stop,
    retry,
  };
}
