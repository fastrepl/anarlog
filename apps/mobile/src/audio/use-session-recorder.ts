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
import { useCallback, useEffect, useRef, useState } from "react";

import { WAVEFORM_BAR_COUNT } from "@/components/waveform";
import { catalogSessionAudio } from "@/data/audio-catalog";
import { transcribeSession } from "@/data/transcribe";
import { captureAnalytics } from "@/lib/analytics";
import { captureOperationalError } from "@/lib/error-reporting";

const METERING_FLOOR_DB = -50;

const CONTENT_TYPES: Record<string, string> = {
  m4a: "audio/mp4",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
  caf: "audio/x-caf",
};

export type RecorderPhase =
  | "idle"
  | "starting"
  | "recording"
  | "saving"
  | "saved"
  | "unavailable"
  | "error";

export type StopResult = "saved" | "failed" | "noop";

export function useSessionRecorder(
  sessionId: string,
  enabled: boolean,
): {
  phase: RecorderPhase;
  levels: number[] | null;
  durationMs: number;
  stop: () => Promise<StopResult>;
} {
  const reportedStatusErrorRef = useRef<string | null>(null);
  const handleRecorderStatus = useCallback((status: RecordingStatus) => {
    const errorKey = status.hasError
      ? (status.error ?? "unknown")
      : status.mediaServicesDidReset
        ? "media_services_reset"
        : null;
    if (!errorKey || reportedStatusErrorRef.current === errorKey) return;

    reportedStatusErrorRef.current = errorKey;
    captureOperationalError(
      new Error(
        status.mediaServicesDidReset
          ? "Audio media services reset"
          : (status.error ?? "Native recording failed"),
      ),
      {
        operation: status.mediaServicesDidReset
          ? "recording_media_services_reset"
          : "recording_native_status",
        level: status.mediaServicesDidReset ? "warning" : "error",
        tags: {
          media_services_reset: status.mediaServicesDidReset === true,
        },
      },
    );
  }, []);
  const recorder = useAudioRecorder(
    {
      ...RecordingPresets.HIGH_QUALITY,
      directory: "document",
      isMeteringEnabled: true,
    },
    handleRecorderStatus,
  );
  const recorderState = useAudioRecorderState(recorder, 50);
  const [phase, setPhase] = useState<RecorderPhase>("idle");
  const [levels, setLevels] = useState<number[] | null>(null);
  const phaseRef = useRef(phase);
  phaseRef.current = phase;
  const startRef = useRef<Promise<void> | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    setPhase("starting");
    startRef.current = (async () => {
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
        if (!active) return;
        if (!permission.granted) {
          captureAnalytics("session_start_failed", {
            failure_stage: "microphone_permission",
          });
          setPhase("unavailable");
          return;
        }
        await setAudioModeAsync({
          allowsRecording: true,
          playsInSilentMode: true,
          // The core use case is a locked phone on the table mid-conversation.
          allowsBackgroundRecording: true,
        });
        await recorder.prepareToRecordAsync();
        if (!active) return;
        recorder.record();
        captureAnalytics("session_started", {
          entry_point: "mobile_recorder",
          transcription_mode: "post_capture",
        });
        setPhase("recording");
      } catch (error) {
        captureOperationalError(error, {
          operation: "recording_start",
          tags: { stage: "capture_start" },
        });
        captureAnalytics("session_start_failed", {
          failure_stage: "capture_start",
        });
        if (active) setPhase("unavailable");
      }
    })();
    return () => {
      active = false;
    };
  }, [recorder, enabled]);

  const metering = recorderState.metering;
  useEffect(() => {
    if (typeof metering !== "number" || phaseRef.current !== "recording") {
      return;
    }
    const level = Math.min(
      1,
      Math.max(0, (metering - METERING_FLOOR_DB) / -METERING_FLOOR_DB),
    );
    setLevels((current) => [
      ...(current ?? Array<number>(WAVEFORM_BAR_COUNT).fill(0)).slice(1),
      level,
    ]);
  }, [metering]);

  const stop = async (): Promise<StopResult> => {
    if (phaseRef.current !== "recording" && phaseRef.current !== "starting") {
      return "noop";
    }
    // Stopping mid-startup has to wait for record() to actually happen,
    // otherwise the recorder is left running with nobody able to stop it.
    await startRef.current?.catch(() => {});
    if (phaseRef.current !== "recording") return "noop";
    setPhase("saving");
    try {
      await recorder.stop();
      const uri = recorder.uri;
      if (!uri) throw new Error("recording produced no file");
      const extension = uri.split(".").pop()?.toLowerCase() ?? "m4a";
      const directory = new Directory(Paths.document, "sessions", sessionId);
      directory.create({ intermediates: true, idempotent: true });
      const destination = new File(directory, `audio.${extension}`);
      if (destination.exists) destination.delete();
      const source = new File(uri);
      await source.move(destination);
      await catalogSessionAudio(sessionId, {
        filename: `audio.${extension}`,
        contentType: CONTENT_TYPES[extension] ?? "application/octet-stream",
        sizeBytes: destination.size ?? 0,
      });
      captureAnalytics("session_completed", {
        duration_seconds: Math.round(
          (recorderState.durationMillis ?? 0) / 1000,
        ),
        completion_reason: "user_stopped",
        transcription_requested: true,
      });
      void transcribeSession(sessionId);
      setPhase("saved");
      return "saved";
    } catch (error) {
      captureOperationalError(error, {
        operation: "recording_save",
        tags: { stage: "persist_audio" },
      });
      setPhase("error");
      return "failed";
    }
  };

  // Gesture and hardware back unmount the screen without reaching its own back
  // handler, so the recorder has to tear itself down or the mic keeps running
  // and the capture is never saved.
  const stopRef = useRef(stop);
  stopRef.current = stop;
  useEffect(
    () => () => {
      void stopRef.current();
    },
    [],
  );

  return {
    phase,
    levels,
    durationMs: recorderState.durationMillis ?? 0,
    stop,
  };
}
