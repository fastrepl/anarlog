import { arch, platform } from "@tauri-apps/plugin-os";
import { useCallback } from "react";

import type { TranscriptionParams } from "@anlg/plugin-transcription";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { BatchResponseProcessingError } from "./batch-response-processing-error";
import { useListener } from "./contexts";
import { persistTranscriptWrite } from "./persist-retry";
import { getSessionKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { withCloudsyncActivity } from "~/db/cloudsync-activity";
import { env } from "~/env";
import {
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
} from "~/services/audio-retention";
import { maybeExtractVoiceprintCandidates } from "~/services/voiceprint";
import { markSessionAudioTranscriptionComplete } from "~/session/attachments";
import { useSession, useSessionParticipants } from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import type { BatchPersistCallback } from "~/store/zustand/listener/transcript";
import {
  getTranscriptionLanguages,
  isDesktopLocalSttAvailable,
  isOnDeviceSttModel,
  isSupportedLanguagesBatch,
} from "~/stt/capabilities";
import {
  createTranscript,
  getTranscriptRecord,
  type TranscriptRecord,
} from "~/stt/queries";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";

type RunOptions = {
  deferAudioFinalization?: boolean;
  handlePersist?: BatchPersistCallback;
  notifyOnCompletion?: boolean;
  provider?: string;
  model?: string;
  baseUrl?: string;
  apiKey?: string;
  keywords?: string[];
  languages?: string[];
  numSpeakers?: number;
  minSpeakers?: number;
  maxSpeakers?: number;
  promotion?:
    | { scope: "preserve_existing" }
    | { scope: "whole_session" }
    | {
        scope: "current_capture";
        audioOffsetMs: number;
        replaceTranscriptId?: string;
        startedAt: number;
      };
};

type BatchTarget = {
  provider: TranscriptionParams["provider"];
  model: string;
  baseUrl: string;
  apiKey: string;
  label: string;
};

const DIRECT_BATCH_PROVIDERS: Set<TranscriptionParams["provider"]> = new Set([
  "deepgram",
  "cartesia",
  "soniox",
  "assemblyai",
  "openai",
  "openrouter",
  "siliconflow",
  "zai",
  "gladia",
  "elevenlabs",
  "mistral",
  "fireworks",
  "pyannote",
  "aquavoice",
  "cohere",
  "aws_transcribe",
  "azure_speech",
  "google_cloud",
  "groq",
  "revai",
  "speechmatics",
  "together",
  "xai",
]);

export const STOPPED_TRANSCRIPTION_ERROR_MESSAGE = "Transcription stopped.";
export const EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE =
  "Batch transcription did not include the current recording.";
const MIN_REFINED_SPEAKER_OVERLAP_RATIO = 0.6;
const LOCAL_SONIQO_BATCH_TARGET = {
  provider: "soniqo",
  model: "soniqo-parakeet-batch",
  baseUrl: "soniqo://local",
  apiKey: "",
  label: "Soniqo batch transcription",
} satisfies BatchTarget;

export function getBatchProvider(
  provider: string,
  model: string,
): TranscriptionParams["provider"] | null {
  if (provider === "cloudflare_workers_ai") {
    return "deepgram";
  }

  if (provider === "anarlog") {
    if (model.startsWith("soniqo-")) return "soniqo";
    if (model === "apple-speech") return "applespeech";
    if (model.startsWith("am-")) return "am";
    return "anarlog";
  }
  if (provider === "soniqo") return "soniqo";
  if (provider === "apple_speech" || provider === "apple-speech") {
    return "applespeech";
  }
  if (DIRECT_BATCH_PROVIDERS.has(provider as TranscriptionParams["provider"])) {
    return provider as TranscriptionParams["provider"];
  }
  return null;
}

export function canRunBatchTranscription(
  _conn: { provider: string; model: string } | null,
  _modelOverride?: string,
) {
  return true;
}

export function getBatchFallbackTarget({
  isPaid,
  accessToken,
  apiBaseUrl,
  currentPlatform = platform(),
  currentArch = arch(),
}: {
  isPaid: boolean;
  accessToken?: string | null;
  apiBaseUrl: string;
  currentPlatform?: ReturnType<typeof platform>;
  currentArch?: ReturnType<typeof arch>;
}): BatchTarget | null {
  if (isPaid && accessToken) {
    return {
      provider: "anarlog",
      model: "cloud",
      baseUrl: new URL("/stt", apiBaseUrl).toString(),
      apiKey: accessToken,
      label: "Pro cloud transcription",
    };
  }

  return isDesktopLocalSttAvailable(currentPlatform, currentArch)
    ? LOCAL_SONIQO_BATCH_TARGET
    : null;
}

async function canUseBatchTarget(
  provider: TranscriptionParams["provider"],
  model: string,
  languages: readonly string[],
) {
  return isSupportedLanguagesBatch(provider, model, languages);
}

function selectedProviderLabel(
  conn: { provider: string; model: string } | null,
  modelOverride?: string,
) {
  if (!conn) {
    return "the selected speech-to-text provider";
  }

  return modelOverride ?? conn.model ?? conn.provider;
}

function sameBatchTarget(
  a: Pick<BatchTarget, "provider" | "model"> | null,
  b: Pick<BatchTarget, "provider" | "model">,
) {
  return a?.provider === b.provider && a.model === b.model;
}

function prepareTranscriptPromotion(
  words: WordWithId[],
  hints: SpeakerHintWithId[],
  promotion: NonNullable<RunOptions["promotion"]>,
) {
  if (promotion.scope !== "current_capture") {
    return {
      words,
      hints,
      replaceSession: promotion.scope === "whole_session",
      replaceTranscriptId: undefined,
      startedAt: undefined,
    };
  }

  const offsetMs = Number.isFinite(promotion.audioOffsetMs)
    ? Math.max(0, promotion.audioOffsetMs)
    : 0;
  const currentWords = words.flatMap((word) => {
    const startMs = word.start_ms ?? 0;
    const endMs = word.end_ms ?? startMs;
    if (endMs <= offsetMs) {
      return [];
    }
    return [
      {
        ...word,
        start_ms: Math.max(0, startMs - offsetMs),
        end_ms: Math.max(0, endMs - offsetMs),
      },
    ];
  });
  const currentWordIds = new Set(currentWords.map((word) => word.id));

  return {
    words: currentWords,
    hints: hints.filter(
      (hint) =>
        typeof hint.word_id === "string" && currentWordIds.has(hint.word_id),
    ),
    replaceSession: false,
    replaceTranscriptId: promotion.replaceTranscriptId,
    startedAt: promotion.startedAt,
  };
}

function parseHintValue(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === "object"
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function speakerKeysByWordId(hints: SpeakerHintWithId[]) {
  const keys = new Map<string, string>();

  for (const hint of hints) {
    if (hint.type !== "provider_speaker_index" || !hint.word_id) {
      continue;
    }

    const value = parseHintValue(hint.value);
    const channel = value?.channel;
    const speakerIndex = value?.speaker_index;
    if (typeof channel !== "number" || typeof speakerIndex !== "number") {
      continue;
    }

    keys.set(hint.word_id, `${channel}:${speakerIndex}`);
  }

  return keys;
}

function parseSpeakerKey(key: string) {
  const separator = key.indexOf(":");
  if (separator <= 0 || separator === key.length - 1) {
    return null;
  }

  const channel = Number(key.slice(0, separator));
  const speakerIndex = Number(key.slice(separator + 1));

  return Number.isFinite(channel) && Number.isFinite(speakerIndex)
    ? { channel, speakerIndex }
    : null;
}

export function reconcileRefinedSpeakerClusters(
  source: TranscriptRecord,
  words: WordWithId[],
  hints: SpeakerHintWithId[],
): SpeakerHintWithId[] {
  const sourceSpeakerKeys = speakerKeysByWordId(source.speakerHints);
  const targetSpeakerKeys = speakerKeysByWordId(hints);
  if (sourceSpeakerKeys.size === 0 || targetSpeakerKeys.size === 0) {
    return hints;
  }

  const sourceIntervalsByChannel = new Map<
    number,
    Array<{
      speakerKey: string;
      startMs: number;
      endMs: number;
    }>
  >();

  for (const word of source.words) {
    const speakerKey = sourceSpeakerKeys.get(word.id);
    const speaker = speakerKey ? parseSpeakerKey(speakerKey) : null;
    if (!speakerKey || !speaker) {
      continue;
    }

    const startMs = word.start_ms ?? 0;
    const endMs = Math.max(startMs + 1, word.end_ms ?? startMs);
    const intervals = sourceIntervalsByChannel.get(speaker.channel) ?? [];
    intervals.push({ speakerKey, startMs, endMs });
    sourceIntervalsByChannel.set(speaker.channel, intervals);
  }

  for (const intervals of sourceIntervalsByChannel.values()) {
    intervals.sort((left, right) => left.startMs - right.startMs);
  }

  const targetWords = words
    .flatMap((word) => {
      const speakerKey = targetSpeakerKeys.get(word.id);
      const speaker = speakerKey ? parseSpeakerKey(speakerKey) : null;
      if (!speakerKey || !speaker) {
        return [];
      }

      const startMs = word.start_ms ?? 0;
      return [
        {
          speakerKey,
          channel: speaker.channel,
          startMs,
          endMs: Math.max(startMs + 1, word.end_ms ?? startMs),
        },
      ];
    })
    .sort(
      (left, right) =>
        left.channel - right.channel || left.startMs - right.startMs,
    );
  const cursors = new Map<number, number>();
  const weights = new Map<string, Map<string, number>>();

  for (const word of targetWords) {
    const sourceIntervals = sourceIntervalsByChannel.get(word.channel);
    if (!sourceIntervals) {
      continue;
    }

    let cursor = cursors.get(word.channel) ?? 0;
    while (
      cursor < sourceIntervals.length &&
      sourceIntervals[cursor].endMs <= word.startMs
    ) {
      cursor += 1;
    }
    cursors.set(word.channel, cursor);

    for (
      let index = cursor;
      index < sourceIntervals.length &&
      sourceIntervals[index].startMs < word.endMs;
      index += 1
    ) {
      const source = sourceIntervals[index];
      const overlapMs =
        Math.min(word.endMs, source.endMs) -
        Math.max(word.startMs, source.startMs);
      if (overlapMs <= 0) {
        continue;
      }

      const sourceWeights = weights.get(word.speakerKey) ?? new Map();
      sourceWeights.set(
        source.speakerKey,
        (sourceWeights.get(source.speakerKey) ?? 0) + overlapMs,
      );
      weights.set(word.speakerKey, sourceWeights);
    }
  }

  const sourceSpeakerByTarget = new Map<string, number>();
  for (const [targetSpeakerKey, sourceWeights] of weights) {
    const candidates = [...sourceWeights].sort(
      ([leftKey, leftWeight], [rightKey, rightWeight]) =>
        rightWeight - leftWeight || leftKey.localeCompare(rightKey),
    );
    const totalOverlapMs = candidates.reduce(
      (total, [, overlapMs]) => total + overlapMs,
      0,
    );
    const [sourceSpeakerKey, overlapMs] = candidates[0] ?? [];
    const sourceSpeaker = sourceSpeakerKey
      ? parseSpeakerKey(sourceSpeakerKey)
      : null;
    if (
      sourceSpeaker &&
      totalOverlapMs > 0 &&
      overlapMs / totalOverlapMs >= MIN_REFINED_SPEAKER_OVERLAP_RATIO
    ) {
      sourceSpeakerByTarget.set(targetSpeakerKey, sourceSpeaker.speakerIndex);
    }
  }

  const usedSpeakerIndicesByChannel = new Map<number, Set<number>>();
  for (const [targetSpeakerKey, speakerIndex] of sourceSpeakerByTarget) {
    const targetSpeaker = parseSpeakerKey(targetSpeakerKey);
    if (!targetSpeaker) {
      continue;
    }

    const usedSpeakerIndices =
      usedSpeakerIndicesByChannel.get(targetSpeaker.channel) ?? new Set();
    usedSpeakerIndices.add(speakerIndex);
    usedSpeakerIndicesByChannel.set(targetSpeaker.channel, usedSpeakerIndices);
  }

  const collidingTargetSpeakers: Array<{
    speakerKey: string;
    channel: number;
  }> = [];
  for (const targetSpeakerKey of new Set(targetSpeakerKeys.values())) {
    if (sourceSpeakerByTarget.has(targetSpeakerKey)) {
      continue;
    }

    const targetSpeaker = parseSpeakerKey(targetSpeakerKey);
    if (!targetSpeaker) {
      continue;
    }

    const usedSpeakerIndices =
      usedSpeakerIndicesByChannel.get(targetSpeaker.channel) ?? new Set();
    if (usedSpeakerIndices.has(targetSpeaker.speakerIndex)) {
      collidingTargetSpeakers.push({
        speakerKey: targetSpeakerKey,
        channel: targetSpeaker.channel,
      });
    } else {
      usedSpeakerIndices.add(targetSpeaker.speakerIndex);
    }
    usedSpeakerIndicesByChannel.set(targetSpeaker.channel, usedSpeakerIndices);
  }

  for (const { speakerKey, channel } of collidingTargetSpeakers) {
    const usedSpeakerIndices = usedSpeakerIndicesByChannel.get(channel);
    if (!usedSpeakerIndices) {
      continue;
    }

    let speakerIndex = Math.max(...usedSpeakerIndices) + 1;
    while (usedSpeakerIndices.has(speakerIndex)) {
      speakerIndex += 1;
    }
    usedSpeakerIndices.add(speakerIndex);
    sourceSpeakerByTarget.set(speakerKey, speakerIndex);
  }

  return hints.map((hint) => {
    if (hint.type !== "provider_speaker_index" || !hint.word_id) {
      return hint;
    }

    const targetSpeakerKey = targetSpeakerKeys.get(hint.word_id);
    const speakerIndex = targetSpeakerKey
      ? sourceSpeakerByTarget.get(targetSpeakerKey)
      : undefined;
    const value = parseHintValue(hint.value);
    if (speakerIndex === undefined || !value) {
      return hint;
    }

    return {
      ...hint,
      value: JSON.stringify({ ...value, speaker_index: speakerIndex }),
    };
  });
}

export function transferAutomaticSpeakerAssignments(
  source: TranscriptRecord,
  words: WordWithId[],
  hints: SpeakerHintWithId[],
  createId: () => string = id,
): SpeakerHintWithId[] {
  if (hints.some((hint) => hint.type === "automatic_speaker_assignment")) {
    return hints;
  }

  const sourceSpeakerKeys = speakerKeysByWordId(source.speakerHints);
  const targetSpeakerKeys = speakerKeysByWordId(hints);
  const sourceIdentityBySpeaker = new Map<
    string,
    { humanId: string; value: string }
  >();

  for (const hint of source.speakerHints) {
    if (hint.type !== "automatic_speaker_assignment" || !hint.word_id) {
      continue;
    }

    const speakerKey = sourceSpeakerKeys.get(hint.word_id);
    const value = parseHintValue(hint.value);
    const humanId = value?.human_id;
    if (speakerKey && typeof humanId === "string" && humanId) {
      sourceIdentityBySpeaker.set(speakerKey, {
        humanId,
        value: hint.value,
      });
    }
  }

  if (sourceIdentityBySpeaker.size === 0 || targetSpeakerKeys.size === 0) {
    return hints;
  }

  const sourceIntervals = source.words.flatMap((word) => {
    const speakerKey = sourceSpeakerKeys.get(word.id);
    const identity = speakerKey
      ? sourceIdentityBySpeaker.get(speakerKey)
      : undefined;
    if (!speakerKey || !identity) {
      return [];
    }

    return [
      {
        speakerKey,
        humanId: identity.humanId,
        value: identity.value,
        startMs: word.start_ms ?? 0,
        endMs: word.end_ms ?? word.start_ms ?? 0,
      },
    ];
  });
  const firstTargetWordBySpeaker = new Map<string, string>();
  const weights = new Map<string, Map<string, number>>();

  for (const word of words) {
    const targetSpeakerKey = targetSpeakerKeys.get(word.id);
    if (!targetSpeakerKey) {
      continue;
    }

    firstTargetWordBySpeaker.set(
      targetSpeakerKey,
      firstTargetWordBySpeaker.get(targetSpeakerKey) ?? word.id,
    );
    const targetStartMs = word.start_ms ?? 0;
    const targetEndMs = word.end_ms ?? targetStartMs;

    for (const sourceWord of sourceIntervals) {
      if (
        sourceWord.speakerKey.split(":", 1)[0] !==
        targetSpeakerKey.split(":", 1)[0]
      ) {
        continue;
      }

      const overlapMs =
        Math.min(targetEndMs, sourceWord.endMs) -
        Math.max(targetStartMs, sourceWord.startMs);
      if (overlapMs <= 0) {
        continue;
      }

      const humanWeights =
        weights.get(targetSpeakerKey) ?? new Map<string, number>();
      humanWeights.set(
        sourceWord.humanId,
        (humanWeights.get(sourceWord.humanId) ?? 0) + overlapMs,
      );
      weights.set(targetSpeakerKey, humanWeights);
    }
  }

  const identityByHumanId = new Map(
    [...sourceIdentityBySpeaker.values()].map((identity) => [
      identity.humanId,
      identity,
    ]),
  );
  const candidates = [...weights].flatMap(([speakerKey, humanWeights]) =>
    [...humanWeights].map(([humanId, overlapMs]) => ({
      speakerKey,
      humanId,
      overlapMs,
    })),
  );
  candidates.sort(
    (left, right) =>
      right.overlapMs - left.overlapMs ||
      left.speakerKey.localeCompare(right.speakerKey) ||
      left.humanId.localeCompare(right.humanId),
  );

  const assignments = new Map<string, string>();
  const assignedHumanIds = new Set<string>();
  for (const candidate of candidates) {
    if (
      assignments.has(candidate.speakerKey) ||
      assignedHumanIds.has(candidate.humanId)
    ) {
      continue;
    }

    assignments.set(candidate.speakerKey, candidate.humanId);
    assignedHumanIds.add(candidate.humanId);
  }

  for (const speakerKey of firstTargetWordBySpeaker.keys()) {
    const identity = sourceIdentityBySpeaker.get(speakerKey);
    if (
      !assignments.has(speakerKey) &&
      identity &&
      !assignedHumanIds.has(identity.humanId)
    ) {
      assignments.set(speakerKey, identity.humanId);
      assignedHumanIds.add(identity.humanId);
    }
  }

  const automaticHints = [...assignments].flatMap(([speakerKey, humanId]) => {
    const wordId = firstTargetWordBySpeaker.get(speakerKey);
    const identity = identityByHumanId.get(humanId);
    if (!wordId || !identity) {
      return [];
    }

    return [
      {
        id: createId(),
        word_id: wordId,
        type: "automatic_speaker_assignment" as const,
        value: identity.value,
      },
    ];
  });

  return [...hints, ...automaticHints];
}

export function isStoppedTranscriptionError(error: unknown) {
  return (
    (error instanceof Error ? error.message : String(error)) ===
    STOPPED_TRANSCRIPTION_ERROR_MESSAGE
  );
}

export function isTranscriptionAuthenticationError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return /authentication failed|invalid_token|unauthorized|\b401\b/i.test(
    message,
  );
}

export function isTerminalTranscriptionError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    error instanceof BatchResponseProcessingError ||
    message === EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE ||
    isTranscriptionAuthenticationError(error) ||
    /corrupt or unsupported|unsupported (?:audio|data)|invalid audio|no speech|empty transcript/i.test(
      message,
    ) ||
    /\b(?:400|403|404|413|415|422)\b|bad request|invalid api key/i.test(message)
  );
}

export function getSessionSpeakerCount(
  participantHumanIds: Iterable<string>,
  selfHumanId?: string | null,
): number | undefined {
  const humanIds = new Set(
    Array.from(participantHumanIds).filter((humanId) => Boolean(humanId)),
  );

  if (typeof selfHumanId === "string" && selfHumanId) {
    humanIds.add(selfHumanId);
  }

  return humanIds.size > 1 ? humanIds.size : undefined;
}

export const useRunBatch = (sessionId: string) => {
  const session = useSession(sessionId);
  const participants = useSessionParticipants(sessionId);

  const startTranscription = useListener((state) => state.startTranscription);
  const { conn } = useSTTConnection();
  const auth = useAuth();
  const billing = useBillingAccess();
  const aiLanguage = useConfigValue("ai_language");
  const spokenLanguages = useConfigValue("spoken_languages");
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const rememberSpeakers = useConfigValue("remember_speakers") === true;

  return useCallback(
    async (filePath: string, options?: RunOptions) => {
      if (!startTranscription) {
        throw new Error(
          "STT connection is not available. Please configure your speech-to-text provider.",
        );
      }

      const languages =
        options?.languages ??
        getTranscriptionLanguages(aiLanguage, spokenLanguages);
      const currentPlatform = platform();
      const currentArch = arch();
      const selectedProviderId = options?.provider ?? conn?.provider;
      const selectedModel = options?.model ?? conn?.model;
      const selectedProvider =
        selectedProviderId && selectedModel
          ? getBatchProvider(selectedProviderId, selectedModel)
          : null;
      const selectedTarget =
        conn && selectedModel && selectedProvider
          ? {
              provider: selectedProvider,
              model: selectedModel,
              baseUrl: options?.baseUrl ?? conn.baseUrl,
              apiKey: options?.apiKey ?? conn.apiKey,
              label: selectedModel,
            }
          : null;
      const selectedTargetSupported =
        selectedTarget &&
        (!isOnDeviceSttModel(selectedProviderId, selectedModel) ||
          isDesktopLocalSttAvailable(currentPlatform, currentArch))
          ? await canUseBatchTarget(
              selectedTarget.provider,
              selectedTarget.model,
              languages,
            )
          : false;
      const fallbackTarget = getBatchFallbackTarget({
        isPaid: billing.isPaid,
        accessToken: auth?.session?.access_token,
        apiBaseUrl: env.VITE_API_URL,
        currentPlatform,
        currentArch,
      });
      const shouldUseSelectedTarget =
        selectedTargetSupported ||
        (fallbackTarget && sameBatchTarget(selectedTarget, fallbackTarget));
      const target = shouldUseSelectedTarget
        ? (selectedTarget ?? fallbackTarget)
        : fallbackTarget;

      if (!target) {
        throw new Error(
          `${selectedProviderLabel(conn, selectedModel)} is not available for batch transcription on this platform. Configure a batch-capable speech-to-text provider.`,
        );
      }

      if (!shouldUseSelectedTarget) {
        sonnerToast.warning("Using a batch transcription provider", {
          description: `${
            selectedTarget
              ? selectedProviderLabel(conn, selectedModel)
              : selectedProviderLabel(conn)
          } is not available for batch transcription. Using ${target.label} instead.`,
        });
      }

      let refinedTranscriptSource: TranscriptRecord | null = null;
      const replaceTranscriptId =
        options?.promotion?.scope === "current_capture"
          ? options.promotion.replaceTranscriptId
          : undefined;
      if (replaceTranscriptId) {
        try {
          refinedTranscriptSource =
            await getTranscriptRecord(replaceTranscriptId);
        } catch (error) {
          console.warn(
            "[runBatch] failed to load automatic speaker assignments",
            error,
          );
        }
      }

      const createdAt = new Date().toISOString();
      const startedAt = Date.now();
      const memoMd = session?.raw_md ?? "";
      const keywords =
        options?.keywords ??
        (await getSessionKeywords({
          sessionId,
          dictionaryTerms,
        }));
      let transcriptId: string | null = null;
      const inferredNumSpeakers =
        options?.numSpeakers === undefined &&
        options?.minSpeakers === undefined &&
        options?.maxSpeakers === undefined
          ? getSessionSpeakerCount(
              participants
                .filter((participant) => participant.source !== "excluded")
                .map((participant) => participant.humanId),
              session?.user_id,
            )
          : undefined;

      const handlePersist: BatchPersistCallback | undefined =
        options?.handlePersist;
      let stagedWords: WordWithId[] = [];
      let stagedHints: SpeakerHintWithId[] = [];
      const resetStagedTranscript = () => {
        transcriptId = null;
        stagedWords = [];
        stagedHints = [];
      };

      const persist =
        handlePersist ??
        ((words, hints, persistOptions) => {
          if (words.length === 0) {
            return;
          }

          const newWords: WordWithId[] = [];
          const newWordIds: string[] = [];

          words.forEach((word) => {
            const wordId = id();

            newWords.push({
              id: wordId,
              text: word.text,
              start_ms: word.start_ms,
              end_ms: word.end_ms,
              channel: word.channel,
              metadata: word.metadata
                ? JSON.stringify(word.metadata)
                : undefined,
            });

            newWordIds.push(wordId);
          });

          const newHints: SpeakerHintWithId[] = [];

          hints.forEach((hint) => {
            if (hint.data.type !== "provider_speaker_index") {
              return;
            }

            const wordId = newWordIds[hint.wordIndex];
            const word = words[hint.wordIndex];

            if (!wordId || !word) {
              return;
            }

            newHints.push({
              id: id(),
              word_id: wordId,
              type: "provider_speaker_index",
              value: JSON.stringify({
                provider: hint.data.provider ?? target.provider,
                channel: hint.data.channel ?? word.channel,
                speaker_index: hint.data.speaker_index,
              }),
            });
          });

          transcriptId ??= id();
          if (persistOptions?.mode === "replace") {
            stagedWords = [];
            stagedHints = [];
          }
          stagedWords.push(...newWords);
          stagedHints.push(...newHints);
        });

      const cloudsyncLeaseKey = `${sessionId}:${id()}`;
      return withCloudsyncActivity(
        "transcription",
        cloudsyncLeaseKey,
        async () => {
          const params: TranscriptionParams = {
            session_id: sessionId,
            provider: target.provider,
            file_path: filePath,
            model: target.model,
            base_url: target.baseUrl,
            api_key: target.apiKey,
            keywords,
            languages,
            num_speakers: options?.numSpeakers ?? inferredNumSpeakers,
            min_speakers: options?.minSpeakers,
            max_speakers: options?.maxSpeakers,
          };

          try {
            await startTranscription(params, {
              handlePersist: persist,
              notifyOnCompletion: options?.notifyOnCompletion,
            });
          } catch (error) {
            if (
              target.provider !== "anarlog" ||
              target.model !== "cloud" ||
              !isTranscriptionAuthenticationError(error)
            ) {
              throw error;
            }

            const refreshedSession = await auth.refreshSession();
            if (!refreshedSession?.access_token) {
              throw error;
            }

            if (!handlePersist) {
              resetStagedTranscript();
            }
            await startTranscription(
              { ...params, api_key: refreshedSession.access_token },
              {
                handlePersist: persist,
                notifyOnCompletion: options?.notifyOnCompletion,
              },
            );
          }

          try {
            if (!handlePersist) {
              const promoted = prepareTranscriptPromotion(
                stagedWords,
                stagedHints,
                options?.promotion ?? { scope: "preserve_existing" },
              );
              if (
                options?.promotion?.scope === "current_capture" &&
                promoted.words.length === 0
              ) {
                throw new Error(EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE);
              }
              if (transcriptId) {
                const completedTranscriptId = transcriptId;
                if (promoted.words.length > 0) {
                  const reconciledSpeakerHints = refinedTranscriptSource
                    ? reconcileRefinedSpeakerClusters(
                        refinedTranscriptSource,
                        promoted.words,
                        promoted.hints,
                      )
                    : promoted.hints;
                  const speakerHints = refinedTranscriptSource
                    ? transferAutomaticSpeakerAssignments(
                        refinedTranscriptSource,
                        promoted.words,
                        reconciledSpeakerHints,
                      )
                    : reconciledSpeakerHints;
                  await persistTranscriptWrite(() =>
                    createTranscript({
                      id: completedTranscriptId,
                      sessionId,
                      ownerUserId: session?.user_id ?? "",
                      createdAt,
                      startedAt: promoted.startedAt ?? startedAt,
                      memo: memoMd,
                      source: "batch_transcription",
                      provider: target.provider,
                      model: target.model,
                      words: promoted.words,
                      speakerHints,
                      replaceSession: promoted.replaceSession,
                      replaceTranscriptId: promoted.replaceTranscriptId,
                    }),
                  );
                  await maybeExtractVoiceprintCandidates({
                    enabled: rememberSpeakers,
                    sessionId,
                    transcriptId: completedTranscriptId,
                    audioPath: filePath,
                  });
                }
              }
              if (!options?.deferAudioFinalization) {
                try {
                  await persistTranscriptWrite(() =>
                    markSessionAudioTranscriptionComplete(sessionId),
                  );
                } catch (error) {
                  console.error(
                    "[runBatch] failed to mark session audio as processed",
                    error,
                  );
                }
              }
            }
            if (!options?.deferAudioFinalization) {
              await deleteProcessedAudioForRetention(audioRetention, sessionId);
            }
          } catch (error) {
            if (
              error instanceof BatchResponseProcessingError ||
              (error instanceof Error &&
                error.message ===
                  EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE)
            ) {
              throw error;
            }
            throw new BatchResponseProcessingError(error);
          }
        },
      );
    },
    [
      conn,
      auth,
      auth?.session?.access_token,
      aiLanguage,
      audioRetention,
      billing.isPaid,
      dictionaryTerms,
      rememberSpeakers,
      session,
      participants,
      spokenLanguages,
      startTranscription,
      sessionId,
    ],
  );
};
