import {
  RecordingPresets,
  requestRecordingPermissionsAsync,
  setAudioModeAsync,
  useAudioRecorder,
  useAudioRecorderState,
} from "expo-audio";
import { useEffect, useState } from "react";

const METERING_FLOOR_DB = -50;

// Returns a rolling buffer of normalized mic levels (newest last), or null when
// no usable audio stream exists (permission denied, metering unsupported) so
// callers can fall back to an idle visualization.
export function useLiveAudioLevels(barCount: number): number[] | null {
  const recorder = useAudioRecorder({
    ...RecordingPresets.HIGH_QUALITY,
    isMeteringEnabled: true,
  });
  const recorderState = useAudioRecorderState(recorder, 50);
  const [levels, setLevels] = useState<number[] | null>(null);

  useEffect(() => {
    let active = true;
    (async () => {
      const permission = await requestRecordingPermissionsAsync();
      if (!active || !permission.granted) return;
      await setAudioModeAsync({
        allowsRecording: true,
        playsInSilentMode: true,
      });
      await recorder.prepareToRecordAsync();
      if (!active) return;
      recorder.record();
    })();
    return () => {
      active = false;
    };
  }, [recorder]);

  const metering = recorderState.metering;
  useEffect(() => {
    if (typeof metering !== "number") return;
    const level = Math.min(
      1,
      Math.max(0, (metering - METERING_FLOOR_DB) / -METERING_FLOOR_DB),
    );
    setLevels((current) => [
      ...(current ?? Array<number>(barCount).fill(0)).slice(1),
      level,
    ]);
  }, [metering, barCount]);

  return levels;
}
