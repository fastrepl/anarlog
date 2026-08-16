import {
  getRecordingPermissionsAsync,
  RecordingPresets,
  type RecordingStatus,
  requestRecordingPermissionsAsync,
  setAudioModeAsync,
  useAudioRecorder,
  useAudioRecorderState,
} from "expo-audio";
import { Directory, File, Paths } from "expo-file-system";
import { useCallback, useRef, useState } from "react";
import { AppState } from "react-native";

import {
  recoverableRecordingUri,
  recorderRecoveryAction,
  recorderStatusFailure,
  shouldHandleRecorderFailure,
  type RecorderPhase,
} from "@/audio/recorder-status";
import { catalogSessionAudio } from "@/data/audio-catalog";
import { transcribeSession } from "@/data/transcribe";
import { captureAnalytics } from "@/lib/analytics";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

const METERING_FLOOR_DB = -50;

const CONTENT_TYPES: Record<string, string> = {
  m4a: "audio/mp4",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
  caf: "audio/x-caf",
};

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
  stop: () => Promise<StopResult>;
  retry: () => Promise<StopResult>;
} {
  const [phase, setPhaseState] = useState<RecorderPhase>("idle");
  const [failure, setFailure] = useState<RecorderFailure | null>(null);
  const phaseRef = useRef<RecorderPhase>("idle");
  const activeRef = useRef(true);
  const pendingUriRef = useRef<string | null>(null);
  const pendingDurationRef = useRef(0);
  const completionReasonRef = useRef<"user_stopped" | "interrupted">(
    "user_stopped",
  );
  const startGenerationRef = useRef(0);
  const startRef = useRef<Promise<void> | null>(null);
  const stopOperationRef = useRef<Promise<StopResult> | null>(null);
  const completionTrackedRef = useRef(false);
  const reportedFailureRef = useRef<string | null>(null);
  const recorderRef = useRef<ReturnType<typeof useAudioRecorder> | null>(null);

  const setPhase = useCallback((next: RecorderPhase) => {
    phaseRef.current = next;
    if (activeRef.current) setPhaseState(next);
  }, []);

  const reportFailure = useCallback(
    (reason: RecorderFailure, error: unknown, operation: string) => {
      if (reportedFailureRef.current !== reason) {
        reportedFailureRef.current = reason;
        captureAnalytics("recording_failed", {
          failure_stage: reason,
        });
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

  const handleRecorderStatus = useCallback(
    (status: RecordingStatus) => {
      const statusFailure = recorderStatusFailure(status);
      if (!statusFailure || !shouldHandleRecorderFailure(phaseRef.current)) {
        return;
      }

      const recoverableUri = recoverableRecordingUri(
        status.url,
        recorderRef.current?.uri,
      );
      if (recoverableUri) pendingUriRef.current = recoverableUri;
      completionReasonRef.current = "interrupted";
      reportFailure(
        statusFailure.reason,
        new Error(statusFailure.message),
        statusFailure.reason === "media_services_reset"
          ? "recording_media_services_reset"
          : "recording_native_status",
      );
      setPhase(statusFailure.phase);
    },
    [reportFailure, setPhase],
  );

  const recorder = useAudioRecorder(
    {
      ...RecordingPresets.HIGH_QUALITY,
      directory: "document",
      isMeteringEnabled: true,
    },
    handleRecorderStatus,
  );
  recorderRef.current = recorder;
  const recorderState = useAudioRecorderState(recorder, 50);

  const persistRecording = useCallback(
    async (sourceUri: string, durationMs: number): Promise<StopResult> => {
      setPhase("saving");
      try {
        const extension = sourceUri.split(".").pop()?.toLowerCase() ?? "m4a";
        const directory = new Directory(Paths.document, "sessions", sessionId);
        directory.create({ intermediates: true, idempotent: true });
        const destination = new File(directory, `audio.${extension}`);
        if (sourceUri !== destination.uri) {
          if (destination.exists) destination.delete();
          const source = new File(sourceUri);
          await source.move(destination);
          pendingUriRef.current = destination.uri;
        }
        await catalogSessionAudio(sessionId, {
          filename: `audio.${extension}`,
          contentType: CONTENT_TYPES[extension] ?? "application/octet-stream",
          sizeBytes: destination.size ?? 0,
        });
        if (!completionTrackedRef.current) {
          completionTrackedRef.current = true;
          captureAnalytics("recording_completed", {
            duration_seconds: Math.round(durationMs / 1_000),
            completion_reason: completionReasonRef.current,
            transcription_requested: true,
          });
          captureAnalytics("session_completed", {
            duration_seconds: Math.round(durationMs / 1_000),
            completion_reason: completionReasonRef.current,
            transcription_requested: true,
          });
        }
        pendingUriRef.current = null;
        setFailure(null);
        void transcribeSession(sessionId);
        setPhase("saved");
        return "saved";
      } catch (error) {
        reportFailure("save_failed", error, "recording_save");
        setPhase("save_error");
        return "failed";
      }
    },
    [reportFailure, sessionId, setPhase],
  );

  const start = useCallback(async () => {
    const generation = ++startGenerationRef.current;
    const isCurrent = () =>
      activeRef.current && generation === startGenerationRef.current;
    pendingDurationRef.current = 0;
    completionReasonRef.current = "user_stopped";
    completionTrackedRef.current = false;
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
      await recorder.prepareToRecordAsync();
      if (!isCurrent()) return;
      recorder.record();
      captureAnalytics("recording_started", {
        entry_point: "mobile_recorder",
        transcription_mode: "post_capture",
      });
      captureAnalytics("session_started", {
        entry_point: "mobile_recorder",
        transcription_mode: "post_capture",
      });
      setPhase("recording");
    } catch (error) {
      reportFailure("start_failed", error, "recording_start");
      captureAnalytics("session_start_failed", {
        failure_stage: "capture_start",
      });
      setPhase("error");
    }
  }, [recorder, reportFailure, setPhase]);

  useMountEffect(() => {
    activeRef.current = true;
    if (!enabled) return;
    startRef.current = start();
  });

  useMountEffect(() => {
    let previousState = AppState.currentState;
    const subscription = AppState.addEventListener("change", (nextState) => {
      const returnedFromSettings =
        previousState !== "active" &&
        nextState === "active" &&
        phaseRef.current === "unavailable";
      previousState = nextState;
      if (returnedFromSettings) {
        startRef.current = start();
      }
    });
    return () => subscription.remove();
  });

  const performStop = async (): Promise<StopResult> => {
    let action = recorderRecoveryAction(
      phaseRef.current,
      pendingUriRef.current !== null,
      "stop",
    );
    const pendingUri = pendingUriRef.current;
    if (action === "persist" && pendingUri) {
      return persistRecording(pendingUri, pendingDurationRef.current);
    }
    if (action !== "stop") return "noop";

    await startRef.current?.catch(() => {});
    action = recorderRecoveryAction(
      phaseRef.current,
      pendingUriRef.current !== null,
      "stop",
    );
    const startedPendingUri = pendingUriRef.current;
    if (action === "persist" && startedPendingUri) {
      return persistRecording(startedPendingUri, pendingDurationRef.current);
    }
    if (phaseRef.current === "starting") return "noop";
    if (action !== "stop") return "noop";

    setPhase("saving");
    pendingDurationRef.current =
      recorderState.durationMillis ?? pendingDurationRef.current;
    try {
      await recorder.stop();
    } catch (error) {
      const uri = recorder.uri;
      if (uri) {
        pendingUriRef.current = uri;
        return persistRecording(uri, pendingDurationRef.current);
      }
      reportFailure("save_failed", error, "recording_stop");
      setPhase("save_error");
      return "failed";
    }
    const uri = recorder.uri;
    if (!uri) {
      reportFailure(
        "save_failed",
        new Error("recording produced no file"),
        "recording_stop",
      );
      setPhase("save_error");
      return "failed";
    }
    pendingUriRef.current = uri;
    return persistRecording(uri, pendingDurationRef.current);
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
    const action = recorderRecoveryAction(
      phaseRef.current,
      pendingUriRef.current !== null,
      "retry",
    );
    if (action === "persist" || action === "stop") {
      return stop();
    }
    if (action === "restart") {
      startRef.current = start();
      await startRef.current;
    }
    return "noop";
  };

  const stopRef = useRef(stop);
  stopRef.current = stop;
  useMountEffect(() => () => {
    activeRef.current = false;
    startGenerationRef.current += 1;
    void stopRef.current();
  });

  const metering = recorderState.metering;
  if (
    phase === "recording" &&
    (recorderState.durationMillis ?? 0) > pendingDurationRef.current
  ) {
    pendingDurationRef.current = recorderState.durationMillis ?? 0;
  }
  const normalizedLevel =
    typeof metering === "number" && phase === "recording"
      ? Math.min(
          1,
          Math.max(0, (metering - METERING_FLOOR_DB) / -METERING_FLOOR_DB),
        )
      : null;
  return {
    phase,
    failure,
    amplitude: normalizedLevel ?? 0,
    durationMs: recorderState.durationMillis ?? pendingDurationRef.current,
    stop,
    retry,
  };
}
