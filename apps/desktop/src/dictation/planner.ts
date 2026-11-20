import { arch, platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";

import {
  commands as dictation,
  type ContextSnapshot,
  type DictationPolicy,
  type PreviewConfig,
} from "@anlg/plugin-dictation";

import { DictationRetired, type DictationPlan } from "./engine";
import {
  addSuggestions,
  parseReplacements,
  parseSuggestions,
} from "./learning";

import { refineDictation } from "~/ai/dictation-refine";
import { useLanguageModel } from "~/ai/hooks/useLLMConnection";
import { useAuth } from "~/auth";
import {
  getStoredSettingValues,
  updateSettingValue,
  useSettingsReady,
} from "~/settings/queries";
import { resolveConfigValue, useConfigValues } from "~/shared/config";
import {
  getTranscriptionLanguages,
  isDesktopLocalSttAvailable,
  isLocalFileSttModel,
  isOnDeviceSttModel,
  isSupportedLanguagesBatch,
} from "~/stt/capabilities";
import { getBatchProvider, useRunBatch } from "~/stt/useRunBatch";
import { useSTTConnection } from "~/stt/useSTTConnection";

const LOCAL_UNAVAILABLE =
  "Your local transcription model can't finish this dictation. Check Settings > Transcription; Anarlog won't send it to a cloud provider instead.";

const SETTING_KEYS = [
  "current_stt_provider",
  "current_stt_model",
  "microphone_device",
  "ai_language",
  "spoken_languages",
  "personalization_dictionary_terms",
  "personalization_dictionary_replacements",
  "dictation_live_preview",
  "dictation_cleanup",
  "dictation_voice_edit",
  "dictation_context",
  "dictation_learning",
  "dictation_ducking",
  "dictation_recovery",
  "dictation_term_weights",
] as const;

let learningWrites: Promise<void> = Promise.resolve();

/** One learning write at a time, each from the stored value it read, so none is lost. */
function serialLearning(task: () => Promise<void>) {
  const next = learningWrites.then(task).catch((error: unknown) => {
    if (!(error instanceof DictationRetired)) throw error;
  });
  learningWrites = next.catch(() => {});
  return next;
}

/**
 * Persists a learning value only if learning still applies when the settings write itself
 * runs: it can wait behind other settings writes while the account or the option changes.
 */
function learningWrite<
  K extends "dictation_term_weights" | "dictation_dictionary_suggestions",
>(
  key: K,
  learning: () => boolean,
  next: (current: string | undefined) => string,
) {
  return updateSettingValue(key, (current) => {
    if (!learning()) throw new DictationRetired();
    return next(current);
  });
}

export type DictationOptions = {
  context: boolean;
  learning: boolean;
  voiceEdit: boolean;
};

/**
 * Captures, per operation, the person's current transcription route, cleanup policy and
 * dictation options. Local transcription stays local: a local route that cannot finish
 * a recording fails instead of reaching a cloud provider.
 */
export function useDictationPlanner(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const stt = useSTTConnection();
  const auth = useAuth();
  const model = useLanguageModel();
  const settingsReady = useSettingsReady();
  const values = useConfigValues(SETTING_KEYS);
  const latest = useRef({
    runBatch,
    stt,
    auth,
    model,
    settingsReady,
    values,
  });
  latest.current = {
    runBatch,
    stt,
    auth,
    model,
    settingsReady,
    values,
  };

  // Learning writes only while the same account is signed in with learning still on.
  const isLearning = (accountId: string) =>
    latest.current.auth.session?.user.id === accountId &&
    latest.current.values.dictation_learning;

  const planner = useRef({
    microphone: () => latest.current.values.microphone_device,

    options: (): DictationOptions => {
      const { values } = latest.current;
      return {
        context: values.dictation_context,
        learning: values.dictation_learning,
        voiceEdit: values.dictation_voice_edit,
      };
    },

    plan: async ({
      accountId,
      context,
    }: {
      accountId: string;
      context: ContextSnapshot | null;
    }): Promise<DictationPlan> => {
      const { stt, auth, model, settingsReady, values } = latest.current;
      const conn = stt.conn;
      const localOnly =
        stt.isLocalModel || (!!conn && isLoopback(conn.baseUrl));
      if (localOnly && !conn) throw new Error(LOCAL_UNAVAILABLE);
      const policy: DictationPolicy = {
        provider: conn?.provider ?? values.current_stt_provider ?? "",
        model: conn?.model ?? values.current_stt_model ?? null,
        localOnly,
      };
      const languages = values.spoken_languages ?? [];
      const batchLanguages = getTranscriptionLanguages(
        values.ai_language,
        values.spoken_languages,
      );
      const dictionary = values.personalization_dictionary_terms ?? [];
      const replacements = parseReplacements(
        values.personalization_dictionary_replacements,
      );
      const weights = values.dictation_learning
        ? values.dictation_term_weights || null
        : null;
      // Ranked while the microphone opens; the stream and batch wait for it.
      const keywords = (async () => {
        try {
          return await dictation.rankDictationTerms({
            accountId,
            nowMs: Date.now(),
            dictionary,
            replacements,
            contextTerms: context?.terms ?? [],
            weights,
            limit: null,
          });
        } catch {
          return dictionary;
        }
      })();
      const textPolicy = settingsReady
        ? {
            cleanup: values.dictation_cleanup,
            voiceEdit: values.dictation_voice_edit,
          }
        : null;
      const isCloudModel = stt.isCloudModel;

      return {
        policy,
        microphoneDevice: values.microphone_device || null,
        recovery: values.dictation_recovery,
        ducking: values.dictation_ducking,
        livePreview: values.dictation_live_preview,
        stream: async (): Promise<PreviewConfig | null> => {
          if (!conn) return null;
          const session = isCloudModel
            ? await auth.getSessionForRequest().catch(() => null)
            : null;
          if (isCloudModel && !session) return null;
          return {
            provider: conn.provider,
            baseUrl: conn.baseUrl,
            apiKey: (isCloudModel ? session?.access_token : conn.apiKey) ?? "",
            params: {
              model: conn.model,
              channels: 1,
              sample_rate: 16000,
              languages,
              keywords: await keywords,
              num_speakers: 1,
              min_speakers: null,
              max_speakers: null,
              custom_query: null,
            },
          };
        },
        transcribe: async (audioPath, signal) => {
          const current = latest.current;
          if (policy.localOnly) {
            const now = current.stt.conn;
            if (
              !conn ||
              !now ||
              now.provider !== conn.provider ||
              now.model !== conn.model ||
              !(await canBatchLocally(conn, batchLanguages))
            )
              throw new Error(LOCAL_UNAVAILABLE);
          }
          signal.throwIfAborted();
          let text = "";
          await current.runBatch(audioPath, {
            signal,
            deferAudioFinalization: true,
            notifyOnCompletion: false,
            numSpeakers: 1,
            keywords: await keywords,
            languages: batchLanguages,
            ...(conn
              ? {
                  provider: conn.provider,
                  model: conn.model,
                  baseUrl: conn.baseUrl,
                  apiKey: conn.apiKey,
                }
              : {}),
            handlePersist: (words) => {
              text = words
                .slice()
                .sort((a, b) => a.start_ms - b.start_ms)
                .map((word) => word.text)
                .join("")
                .replace(/\s+/gu, " ")
                .trim();
            },
          });
          return text;
        },
        refine: async (request, signal) =>
          refineDictation({
            model,
            signal,
            request: {
              operationId: request.operationId,
              nowMs: Date.now(),
              text: request.text,
              selection: textPolicy?.voiceEdit ? request.selection : null,
              policy: textPolicy,
              modelReady: model !== null,
              terms: await keywords,
              replacements,
              instructions: null,
              before: request.before,
              context: request.context,
            },
          }),
      };
    },

    /** Weighs the terms of a delivered dictation; only when learning is on. */
    used: (accountId: string, text: string) =>
      serialLearning(async () => {
        const learning = () => isLearning(accountId);
        if (!learning()) return;
        const stored = await getStoredSettingValues();
        const weights = await dictation.recordDictationTerms({
          accountId,
          nowMs: Date.now(),
          dictionary: resolveConfigValue(
            "personalization_dictionary_terms",
            stored,
          ),
          weights: resolveConfigValue("dictation_term_weights", stored) || null,
          event: { type: "used", text },
        });
        if (!learning()) return;
        await learningWrite("dictation_term_weights", learning, () => weights);
      }),

    /** Suggests dictionary terms from what the person kept of this dictation. */
    kept: (
      accountId: string,
      inserted: string,
      kept: string,
      isCurrent: () => boolean,
    ) =>
      serialLearning(async () => {
        const learning = () => isCurrent() && isLearning(accountId);
        if (!learning() || kept === inserted.trim()) return;
        const stored = await getStoredSettingValues();
        const dictionary = resolveConfigValue(
          "personalization_dictionary_terms",
          stored,
        );
        const current = parseSuggestions(
          resolveConfigValue("dictation_dictionary_suggestions", stored),
          accountId,
        )!;
        const terms = await dictation.suggestDictationTerms({
          inserted,
          kept,
          dictionary,
          seen: [...current.terms, ...current.dismissed],
        });
        if (!terms.length || !learning()) return;
        const weights = await dictation.recordDictationTerms({
          accountId,
          nowMs: Date.now(),
          dictionary,
          weights: resolveConfigValue("dictation_term_weights", stored) || null,
          event: { type: "corrected", terms },
        });
        if (!learning()) return;
        // Read-modify-write, so a suggestion settled meanwhile in Dictionary stays settled.
        await learningWrite(
          "dictation_dictionary_suggestions",
          learning,
          (value) =>
            JSON.stringify(
              addSuggestions(parseSuggestions(value, accountId)!, terms),
            ),
        );
        await learningWrite("dictation_term_weights", learning, () => weights);
      }),
  }).current;

  return planner;
}

async function canBatchLocally(
  conn: { provider: string; model: string },
  languages: readonly string[],
) {
  const provider = getBatchProvider(conn.provider, conn.model);
  if (!provider) return false;
  if (
    (isOnDeviceSttModel(conn.provider, conn.model) ||
      isLocalFileSttModel(conn.provider, conn.model)) &&
    !isDesktopLocalSttAvailable(platform(), arch())
  )
    return false;
  return isSupportedLanguagesBatch(provider, conn.model, languages);
}

function isLoopback(baseUrl: string) {
  try {
    const host = new URL(baseUrl).hostname;
    return (
      host === "localhost" ||
      host === "127.0.0.1" ||
      host === "[::1]" ||
      host === "::1"
    );
  } catch {
    return false;
  }
}
