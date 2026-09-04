import { play, type SoundName } from "cuelume";

import { getStoredSettingValues } from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";

export const COMPLETION_SOUND_NAMES = [
  "ready",
  "success",
  "chime",
  "sparkle",
  "bloom",
] as const satisfies readonly SoundName[];

export type CompletionSoundName = (typeof COMPLETION_SOUND_NAMES)[number];

const DEFAULT_COMPLETION_SOUND: CompletionSoundName = "ready";
const COMPLETION_SOUND_VOLUME = 0.7;

export function normalizeCompletionSoundName(
  value: string,
): CompletionSoundName {
  return COMPLETION_SOUND_NAMES.includes(value as CompletionSoundName)
    ? (value as CompletionSoundName)
    : DEFAULT_COMPLETION_SOUND;
}

export function previewCompletionSound(sound: CompletionSoundName): void {
  play(sound, { volume: COMPLETION_SOUND_VOLUME });
}

export async function playCompletionSound(): Promise<void> {
  try {
    const stored = await getStoredSettingValues();
    if (
      resolveConfigValue("notification_disabled", stored) ||
      !resolveConfigValue("notification_completion_sound", stored)
    ) {
      return;
    }

    previewCompletionSound(
      normalizeCompletionSoundName(
        resolveConfigValue("notification_completion_sound_name", stored),
      ),
    );
  } catch (error) {
    console.error("[completion-sound] failed to play completion sound", error);
  }
}
