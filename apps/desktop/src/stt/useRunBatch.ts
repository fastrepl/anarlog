import { t } from "@lingui/core/macro";
import { arch, platform } from "@tauri-apps/plugin-os";
import { useCallback } from "react";

import {
  commands as transcriptionCommands,
  type BatchRefinementSource,
  type BatchProvider,
  type BatchTranscriptPromotion,
  type StoredSpeakerHint,
  type StoredTranscriptWord,
  type TranscriptionParams,
} from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";

import { BatchResponseProcessingError } from "./batch-response-processing-error";
import { completeCaptureTranscript } from "./capture-completion";
import { useListener } from "./contexts";
import { persistTranscriptWrite } from "./persist-retry";
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
import { useSession, useSessionParticipants } from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import {
  acknowledgeCompletedBatch,
  notifyBatchCompleted,
} from "~/store/zustand/listener/general-batch";
import type { BatchPersistCallback } from "~/store/zustand/listener/transcript";
import { serializeBatchResumeContext } from "~/stt/batch-resume-context";
import {
  getTranscriptionLanguages,
  isDesktopLocalSttAvailable,
  isLocalFileSttModel,
  isOnDeviceSttModel,
  isSupportedLanguagesBatch,
} from "~/stt/capabilities";
import type { TranscriptRecord } from "~/stt/queries";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";

type RunOptions = {
  signal?: AbortSignal;
  recovery?: {
    persist: (words: WordWithId[], hints: SpeakerHintWithId[]) => Promise<void>;
  };
  deferAudioFinalization?: boolean;
  handlePersist?: BatchPersistCallback;
  notifyOnCompletion?: boolean;
  resume?: { provider: BatchProvider; model: string };
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
  captureInterval?: { startedAtMs: number; stoppedAtMs: number };
};

type BatchTarget = {
  provider: TranscriptionParams["provider"];
  model: string;
  baseUrl: string;
  apiKey: string;
  label: string;
};

function toStoredTranscriptWord(word: WordWithId): StoredTranscriptWord {
  return {
    ...word,
    metadata: word.metadata as StoredTranscriptWord["metadata"],
  };
}

function toStoredSpeakerHint(hint: SpeakerHintWithId): StoredSpeakerHint {
  return {
    id: hint.id,
    word_id: hint.word_id ?? undefined,
    type: hint.type ?? "",
    value: hint.value as StoredSpeakerHint["value"],
  };
}

function toSpeakerHintWithId(hint: StoredSpeakerHint): SpeakerHintWithId {
  return {
    id: hint.id,
    word_id: hint.word_id ?? "",
    type: hint.type,
    value: hint.value as SpeakerHintWithId["value"],
  };
}

function toBatchRefinementSource(
  transcript: TranscriptRecord,
): BatchRefinementSource {
  return {
    id: transcript.id,
    started_at: transcript.startedAt,
    words: transcript.words.map(toStoredTranscriptWord),
    speaker_hints: transcript.speakerHints.map(toStoredSpeakerHint),
  };
}

export async function reconcileRefinedSpeakerClusters(
  source: TranscriptRecord,
  words: WordWithId[],
  hints: SpeakerHintWithId[],
): Promise<SpeakerHintWithId[]> {
  const result = await transcriptionCommands.reconcileRefinedSpeakerClusters({
    source: toBatchRefinementSource(source),
    words: words.map(toStoredTranscriptWord),
    hints: hints.map(toStoredSpeakerHint),
  });
  if (result.status === "error") throw new Error(result.error);
  return result.data.map(toSpeakerHintWithId);
}

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
  "meta",
  "pyannote",
  "aquavoice",
  "cohere",
  "aws_transcribe",
  "azure_speech",
  "google_cloud",
  "google_generative_ai",
  "groq",
  "revai",
  "speechmatics",
  "together",
  "xai",
  "smallestai",
  "wisprflow",
]);

const STOPPED_TRANSCRIPTION_ERROR_MESSAGE = "Transcription stopped.";
export const EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE =
  "Batch transcription did not include the current recording.";
const INCOMPLETE_BATCH_TRANSCRIPT_ERROR_MESSAGE =
  "The new transcription returned much less text. Your saved transcript and recording were kept. Try transcribing again.";
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
  if (provider === "amazon_bedrock") return "openai";

  if (provider === "custom" || provider === "cloudflare_workers_ai") {
    return "deepgram";
  }

  if (isLocalFileSttModel(provider, model)) {
    return "whispercpp";
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

export function isStoppedTranscriptionError(error: unknown) {
  return (
    (error instanceof Error ? error.message : String(error)) ===
    STOPPED_TRANSCRIPTION_ERROR_MESSAGE
  );
}

function isTranscriptionAuthenticationError(error: unknown) {
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
    message === INCOMPLETE_BATCH_TRANSCRIPT_ERROR_MESSAGE ||
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
  const stopTranscription = useListener((state) => state.stopTranscription);
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
      options?.signal?.throwIfAborted();
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
      const selectedOnDeviceUnsupported = !!(
        selectedTarget &&
        (isOnDeviceSttModel(selectedProviderId, selectedModel) ||
          isLocalFileSttModel(selectedProviderId, selectedModel)) &&
        !isDesktopLocalSttAvailable(currentPlatform, currentArch)
      );
      const selectedTargetSupported =
        selectedTarget && !selectedOnDeviceUnsupported
          ? await canUseBatchTarget(
              selectedTarget.provider,
              selectedTarget.model,
              languages,
            )
          : false;
      options?.signal?.throwIfAborted();
      const requiresCloudSession =
        billing.isPaid ||
        (selectedTarget?.provider === "anarlog" &&
          selectedTarget.model === "cloud");
      const requestSession = requiresCloudSession
        ? await auth.getSessionForRequest().catch(() => null)
        : null;
      options?.signal?.throwIfAborted();
      const cloudAccessToken =
        requestSession?.access_token ?? auth.session?.access_token;
      const fallbackTarget = getBatchFallbackTarget({
        isPaid: billing.isPaid,
        accessToken: cloudAccessToken,
        apiBaseUrl: env.VITE_AI_API_URL ?? env.VITE_API_URL,
        currentPlatform,
        currentArch,
      });
      const shouldUseSelectedTarget =
        selectedTargetSupported ||
        (fallbackTarget && sameBatchTarget(selectedTarget, fallbackTarget));
      let target = options?.resume
        ? {
            provider: options.resume.provider,
            model: options.resume.model,
            baseUrl: conn?.baseUrl ?? "",
            apiKey: conn?.apiKey ?? "",
            label: options.resume.model,
          }
        : shouldUseSelectedTarget
          ? (selectedTarget ?? fallbackTarget)
          : fallbackTarget;

      if (!target) {
        throw new Error(
          selectedTarget && !selectedOnDeviceUnsupported
            ? `${selectedProviderLabel(conn, selectedModel)} is not available for batch transcription with the selected languages. Choose languages it supports, or configure another speech-to-text provider.`
            : `${selectedProviderLabel(conn, selectedModel)} is not available for batch transcription on this platform. Configure a batch-capable speech-to-text provider.`,
        );
      }

      if (target.provider === "anarlog" && target.model === "cloud") {
        if (!cloudAccessToken) {
          throw new Error(t`Transcription failed`);
        }
        target = { ...target, apiKey: cloudAccessToken };
      }

      if (!shouldUseSelectedTarget && !options?.recovery && !options?.resume) {
        toast.warning("Using a batch transcription provider", {
          description: `${
            selectedTarget
              ? selectedProviderLabel(conn, selectedModel)
              : selectedProviderLabel(conn)
          } is not available for batch transcription. Using ${target.label} instead.`,
        });
      }

      const createdAt = new Date().toISOString();
      const startedAt = options?.captureInterval?.startedAtMs ?? Date.now();
      const memoMd = session?.raw_md ?? "";
      let keywords = options?.keywords;
      if (keywords === undefined) {
        const { getSessionKeywords } = await import("./useKeywords");
        keywords = await getSessionKeywords({
          sessionId,
          dictionaryTerms,
        });
      }
      options?.signal?.throwIfAborted();
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
          const jobId = options?.recovery ? `${sessionId}:recovery` : sessionId;
          const params: TranscriptionParams = {
            session_id: jobId,
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
            resume_context:
              !handlePersist &&
              !options?.recovery &&
              !options?.deferAudioFinalization &&
              options?.promotion?.scope === "whole_session"
                ? serializeBatchResumeContext({ promotion: "whole_session" })
                : null,
          };

          const run = async (params: TranscriptionParams) => {
            options?.signal?.throwIfAborted();
            try {
              await startTranscription(params, {
                signal: options?.signal,
                handlePersist: (...args) => {
                  if (options?.signal?.aborted) return;
                  return persist(...args);
                },
                notifyOnCompletion: false,
                recovery: Boolean(options?.recovery),
              });
              options?.signal?.throwIfAborted();
            } finally {
              if (options?.signal?.aborted) {
                await stopTranscription(jobId).catch(() => {});
              }
            }
          };
          try {
            await run(params);
          } catch (error) {
            options?.signal?.throwIfAborted();
            if (
              !(
                options?.recovery &&
                error instanceof Error &&
                error.message === "No speech was detected in the audio."
              )
            ) {
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
              await run({ ...params, api_key: refreshedSession.access_token });
            }
          }

          if (options?.recovery) {
            options.signal?.throwIfAborted();
            await options.recovery.persist(stagedWords, stagedHints);
            await acknowledgeCompletedBatch(jobId);
            return;
          }

          try {
            if (!handlePersist) {
              const promotion = options?.promotion ?? {
                scope: "preserve_existing",
              };
              const refinementPromotion: BatchTranscriptPromotion =
                promotion.scope === "current_capture"
                  ? {
                      scope: "current_capture",
                      audio_offset_ms: promotion.audioOffsetMs,
                      replace_transcript_id: promotion.replaceTranscriptId,
                      started_at: promotion.startedAt,
                    }
                  : { scope: promotion.scope };
              const saved = await persistTranscriptWrite(async () => {
                const result = await transcriptionCommands.saveBatchTranscript({
                  session_id: sessionId,
                  transcript_id: transcriptId,
                  owner_user_id: session?.user_id ?? "",
                  created_at: createdAt,
                  started_at: startedAt,
                  memo: memoMd,
                  provider: target.provider,
                  model: target.model,
                  words: stagedWords.map(toStoredTranscriptWord),
                  hints: stagedHints.map(toStoredSpeakerHint),
                  promotion: refinementPromotion,
                  mark_audio_complete: !options?.deferAudioFinalization,
                });
                if (result.status === "error") {
                  throw new Error(result.error);
                }
                return result.data;
              });
              if (saved.status === "empty_current_capture") {
                throw new Error(EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE);
              }
              if (saved.status === "truncated") {
                throw new Error(INCOMPLETE_BATCH_TRANSCRIPT_ERROR_MESSAGE);
              }
              if (saved.status === "saved" && saved.transcript_id) {
                if (options?.captureInterval) {
                  await persistTranscriptWrite(() =>
                    completeCaptureTranscript(
                      sessionId,
                      saved.transcript_id!,
                      options.captureInterval!.stoppedAtMs,
                    ),
                  );
                }
                await maybeExtractVoiceprintCandidates({
                  enabled: rememberSpeakers,
                  sessionId,
                  transcriptId: saved.transcript_id,
                  audioPath: filePath,
                });
              }
            }
            if (!options?.deferAudioFinalization) {
              await deleteProcessedAudioForRetention(audioRetention, sessionId);
            }
          } catch (error) {
            if (
              error instanceof BatchResponseProcessingError ||
              (error instanceof Error &&
                (error.message ===
                  EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE ||
                  error.message === INCOMPLETE_BATCH_TRANSCRIPT_ERROR_MESSAGE))
            ) {
              throw error;
            }
            throw new BatchResponseProcessingError(error);
          }
          await acknowledgeCompletedBatch(jobId);
          if (options?.notifyOnCompletion !== false) {
            await notifyBatchCompleted(sessionId);
          }
        },
      );
    },
    [
      conn,
      auth,
      auth.session?.access_token,
      aiLanguage,
      audioRetention,
      billing.isPaid,
      dictionaryTerms,
      rememberSpeakers,
      session,
      participants,
      spokenLanguages,
      startTranscription,
      stopTranscription,
      sessionId,
    ],
  );
};
