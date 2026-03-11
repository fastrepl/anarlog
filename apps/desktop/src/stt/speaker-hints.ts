import {
  type ProviderSpeakerIndexHint,
  providerSpeakerIndexSchema,
  type SpeakerHintStorage,
} from "@hypr/store";

import type { RuntimeSpeakerHint } from "./segment";

export type { ProviderSpeakerIndexHint };

export function convertStorageHintsToRuntime(
  storageHints: SpeakerHintStorage[],
  wordIdToIndex: Map<string, number>,
): RuntimeSpeakerHint[] {
  const hints: RuntimeSpeakerHint[] = [];

  storageHints.forEach((hint) => {
    if (typeof hint.word_id !== "string") {
      return;
    }

    const wordIndex = wordIdToIndex.get(hint.word_id);
    if (typeof wordIndex !== "number") {
      return;
    }

    if (hint.type === "provider_speaker_index") {
      const parsed = parseProviderSpeakerIndex(hint.value);
      if (parsed) {
        hints.push({
          wordIndex,
          data: {
            type: "provider_speaker_index",
            speaker_index: parsed.speaker_index,
            provider: parsed.provider,
            channel: parsed.channel,
          },
        });
      }
    } else if (hint.type === "user_speaker_assignment") {
      const data =
        typeof hint.value === "string"
          ? (() => {
              try {
                return JSON.parse(hint.value);
              } catch {
                return undefined;
              }
            })()
          : hint.value;

      if (
        data &&
        typeof data === "object" &&
        "human_id" in data &&
        typeof data.human_id === "string"
      ) {
        hints.push({
          wordIndex,
          data: {
            type: "user_speaker_assignment",
            human_id: data.human_id,
          },
        });
      }
    }
  });

  return hints;
}

const parseProviderSpeakerIndex = (
  raw: unknown,
): ProviderSpeakerIndexHint | undefined => {
  if (raw == null) {
    return undefined;
  }

  const data =
    typeof raw === "string"
      ? (() => {
          try {
            return JSON.parse(raw);
          } catch {
            return undefined;
          }
        })()
      : raw;

  return providerSpeakerIndexSchema.safeParse(data).data;
};
