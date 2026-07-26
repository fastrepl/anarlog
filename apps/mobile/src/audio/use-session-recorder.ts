import {
  RecordingPresets,
  requestRecordingPermissionsAsync,
  setAudioModeAsync,
  useAudioRecorder,
  useAudioRecorderState,
} from "expo-audio";
import { Directory, File, Paths } from "expo-file-system";
import { useEffect, useRef, useState } from "react";

import { WAVEFORM_BAR_COUNT } from "@/components/waveform";
import { catalogSessionAudio } from "@/data/audio-catalog";
import { transcribeSession } from "@/data/transcribe";

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
  const recorder = useAudioRecorder({
    ...RecordingPresets.HIGH_QUALITY,
    isMeteringEnabled: true,
  });
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
        const permission = await requestRecordingPermissionsAsync();
        if (!active) return;
        if (!permission.granted) {
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
        setPhase("recording");
      } catch (error) {
        console.warn("[recorder] failed to start", error);
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
      void transcribeSession(sessionId);
      setPhase("saved");
      return "saved";
    } catch (error) {
      console.warn("[recorder] failed to save recording", error);
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
