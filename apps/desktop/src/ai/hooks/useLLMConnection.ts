import { createAnthropic } from "@ai-sdk/anthropic";
import { createAzure } from "@ai-sdk/azure";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import {
  defaultSettingsMiddleware,
  extractReasoningMiddleware,
  wrapLanguageModel,
} from "ai";
import { useMemo, useRef } from "react";

import type { CharTask } from "@anlg/api-client";
import type { AIProviderStorage } from "@anlg/store";

import { createAppleFoundationModel } from "../apple-foundation-model";
import { createAuthFetch } from "../auth-fetch";
import {
  normalizeReasoningEffort,
  type ReasoningEffort,
  reasoningProviderOptions,
} from "../reasoning-effort";
import { streamOnlyGenerationMiddleware } from "../stream-only-generation";
import { createTracedFetch, tracedFetch } from "../traced-fetch";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { env } from "~/env";
import { type ProviderId, PROVIDERS } from "~/settings/ai/llm/shared";
import {
  CHATGPT_API_BASE_URL,
  createSubscriptionFetch,
  usesSubscriptionFetch,
} from "~/settings/ai/llm/subscriptions";
import {
  getProviderSelectionBlockers,
  type ProviderEligibilityContext,
} from "~/settings/ai/shared/eligibility";
import { useAiProvider } from "~/settings/providers";
import { useConfigValues } from "~/shared/config";

type LanguageModelV3 = Parameters<typeof wrapLanguageModel>[0]["model"];

type LLMConnectionInfo = {
  providerId: ProviderId;
  modelId: string;
  baseUrl: string;
  apiKey: string;
  reasoningEffort: ReasoningEffort;
};

export type LLMConnectionStatus =
  | { status: "pending"; reason: "missing_provider" }
  | { status: "pending"; reason: "missing_model"; providerId: ProviderId }
  | { status: "error"; reason: "provider_not_found"; providerId: string }
  | { status: "error"; reason: "unauthenticated"; providerId: "anarlog" }
  | { status: "error"; reason: "not_pro"; providerId: "anarlog" }
  | {
      status: "error";
      reason: "missing_config";
      providerId: ProviderId;
      missing: Array<"base_url" | "api_key">;
    }
  | { status: "success"; providerId: ProviderId; isHosted: boolean };

type LLMConnectionResult = {
  conn: LLMConnectionInfo | null;
  status: LLMConnectionStatus;
};

export const normalizeLLMProviderId = (providerId: string): string =>
  providerId === "hyprnote" ? "anarlog" : providerId;

export const useLanguageModel = (task?: CharTask): LanguageModelV3 | null => {
  const { conn } = useLLMConnection();
  const auth = useAuth();

  // Auth is resolved at fetch time (not model construction) so token
  // refreshes take effect without recreating the chat transport chain.
  const getSessionForRequestRef = useRef(auth.getSessionForRequest);
  getSessionForRequestRef.current = auth.getSessionForRequest;
  const refreshSessionRef = useRef(auth.refreshSession);
  refreshSessionRef.current = auth.refreshSession;

  return useMemo(() => {
    if (!conn) return null;

    const hostedFetch =
      conn.providerId === "anarlog"
        ? createAuthFetch(
            task ? createTracedFetch(task) : tracedFetch,
            async () => (await getSessionForRequestRef.current())?.access_token,
            async () => (await refreshSessionRef.current())?.access_token,
          )
        : undefined;

    return createLanguageModel(conn, task, hostedFetch);
  }, [conn, task]);
};

export const useLLMConnection = (): LLMConnectionResult => {
  const auth = useAuth();
  // Only the session feeds the connection; the auth object itself changes
  // identity on refresh-mutation state and would churn the model chain.
  const session = auth?.session;
  const billing = useBillingAccess();

  const {
    current_llm_provider,
    current_llm_model,
    current_llm_reasoning_effort,
  } = useConfigValues([
    "current_llm_provider",
    "current_llm_model",
    "current_llm_reasoning_effort",
  ] as const);
  const providerConfig = useAiProvider("llm", current_llm_provider) as
    | AIProviderStorage
    | undefined;

  return useMemo<LLMConnectionResult>(
    () =>
      resolveLLMConnection({
        providerId: current_llm_provider,
        modelId: current_llm_model,
        reasoningEffort: normalizeReasoningEffort(current_llm_reasoning_effort),
        providerConfig,
        session,
        isPaid: billing.isPaid,
      }),
    [
      session,
      billing.isPaid,
      current_llm_model,
      current_llm_provider,
      current_llm_reasoning_effort,
      providerConfig,
    ],
  );
};

export const useLLMConnectionStatus = (): LLMConnectionStatus => {
  const { status } = useLLMConnection();
  return status;
};

const resolveLLMConnection = (params: {
  providerId: string | undefined;
  modelId: string | undefined;
  reasoningEffort: ReasoningEffort;
  providerConfig: AIProviderStorage | undefined;
  session: { access_token: string } | null | undefined;
  isPaid: boolean;
}): LLMConnectionResult => {
  const {
    providerId: rawProviderId,
    modelId,
    reasoningEffort,
    providerConfig,
    session,
    isPaid,
  } = params;

  if (!rawProviderId) {
    return {
      conn: null,
      status: { status: "pending", reason: "missing_provider" },
    };
  }

  const providerId = normalizeLLMProviderId(rawProviderId) as ProviderId;

  if (!modelId) {
    return {
      conn: null,
      status: { status: "pending", reason: "missing_model", providerId },
    };
  }

  const providerDefinition = PROVIDERS.find((p) => p.id === providerId);

  if (!providerDefinition) {
    return {
      conn: null,
      status: {
        status: "error",
        reason: "provider_not_found",
        providerId: rawProviderId,
      },
    };
  }

  const baseUrl =
    providerConfig?.base_url?.trim() ||
    providerDefinition.baseUrl?.trim() ||
    "";
  const apiKey = providerConfig?.api_key?.trim() || "";

  const context: ProviderEligibilityContext = {
    isAuthenticated: !!session,
    isPaid,
    config: { base_url: baseUrl, api_key: apiKey },
  };

  const blockers = getProviderSelectionBlockers(
    providerDefinition.requirements,
    context,
  );

  if (blockers.length > 0) {
    const blocker = blockers[0];
    if (blocker.code === "requires_auth" && providerId === "anarlog") {
      return {
        conn: null,
        status: { status: "error", reason: "unauthenticated", providerId },
      };
    }
    if (blocker.code === "requires_entitlement" && providerId === "anarlog") {
      return {
        conn: null,
        status: { status: "error", reason: "not_pro", providerId },
      };
    }
    if (blocker.code === "missing_config") {
      return {
        conn: null,
        status: {
          status: "error",
          reason: "missing_config",
          providerId,
          missing: blocker.fields,
        },
      };
    }
  }

  if (providerId === "anarlog" && session) {
    return {
      conn: {
        providerId,
        modelId,
        baseUrl: baseUrl ?? new URL("/llm", env.VITE_API_URL).toString(),
        apiKey: session.access_token,
        reasoningEffort,
      },
      status: { status: "success", providerId, isHosted: true },
    };
  }

  return {
    conn: { providerId, modelId, baseUrl, apiKey, reasoningEffort },
    status: { status: "success", providerId, isHosted: false },
  };
};

const wrapWithThinkingMiddleware = (
  model: LanguageModelV3,
): LanguageModelV3 => {
  return wrapLanguageModel({
    model,
    middleware: [
      extractReasoningMiddleware({ tagName: "think" }),
      extractReasoningMiddleware({ tagName: "thinking" }),
    ],
  });
};

const createLanguageModel = (
  conn: LLMConnectionInfo,
  task?: CharTask,
  hostedFetch?: typeof fetch,
): LanguageModelV3 => {
  const model = createProviderModel(conn, task, hostedFetch);
  const providerOptions = reasoningProviderOptions(
    conn.providerId,
    conn.modelId,
    conn.reasoningEffort,
  );
  if (!providerOptions) {
    return model;
  }

  return wrapLanguageModel({
    model,
    middleware: defaultSettingsMiddleware({ settings: { providerOptions } }),
  });
};

const createProviderModel = (
  conn: LLMConnectionInfo,
  task?: CharTask,
  hostedFetch?: typeof fetch,
): LanguageModelV3 => {
  switch (conn.providerId) {
    case "anarlog": {
      const provider = createOpenRouter({
        fetch: hostedFetch ?? (task ? createTracedFetch(task) : tracedFetch),
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider.chat(conn.modelId));
    }

    case "anthropic": {
      const provider = createAnthropic({
        fetch: tauriFetch,
        apiKey: conn.apiKey,
        headers: {
          "anthropic-version": "2023-06-01",
          "anthropic-dangerous-direct-browser-access": "true",
        },
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "claude": {
      const oauth = usesSubscriptionFetch(conn.providerId, conn.apiKey);
      const provider = createAnthropic({
        fetch: oauth
          ? createSubscriptionFetch(conn.providerId, conn.apiKey)
          : tauriFetch,
        apiKey: oauth ? "oauth" : conn.apiKey,
        headers: {
          "anthropic-version": "2023-06-01",
          "anthropic-dangerous-direct-browser-access": "true",
        },
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "chatgpt": {
      const oauth = usesSubscriptionFetch(conn.providerId, conn.apiKey);
      const provider = createOpenAI({
        fetch: oauth
          ? createSubscriptionFetch(conn.providerId, conn.apiKey)
          : tauriFetch,
        baseURL: oauth ? CHATGPT_API_BASE_URL : conn.baseUrl,
        apiKey: oauth ? "oauth" : conn.apiKey,
      });
      const model = provider.responses(conn.modelId);
      return wrapWithThinkingMiddleware(
        oauth
          ? wrapLanguageModel({
              model,
              middleware: streamOnlyGenerationMiddleware,
            })
          : model,
      );
    }

    case "grok":
    case "github_copilot": {
      const provider = createOpenAICompatible({
        fetch: createSubscriptionFetch(conn.providerId, conn.apiKey),
        name: conn.providerId,
        baseURL: conn.baseUrl,
        apiKey: "oauth",
      });
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }

    case "google_generative_ai": {
      const provider = createGoogleGenerativeAI({
        fetch: tauriFetch,
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "openrouter": {
      const provider = createOpenRouter({
        fetch: tauriFetch,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider.chat(conn.modelId));
    }

    case "openai": {
      const provider = createOpenAI({
        fetch: tauriFetch,
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "azure_openai": {
      const provider = createAzure({
        fetch: tauriFetch,
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "azure_ai": {
      const provider = createOpenAICompatible({
        fetch: tauriFetch,
        name: "azure_ai",
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
        headers: { "api-key": conn.apiKey },
      });
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }

    case "ollama": {
      const ollamaOrigin = new URL(conn.baseUrl.replace(/\/v1\/?$/, "")).origin;
      const ollamaFetch: typeof fetch = async (input, init) => {
        const headers = new Headers(init?.headers);
        headers.set("Origin", ollamaOrigin);
        return tauriFetch(input as RequestInfo | URL, {
          ...init,
          headers,
        });
      };
      const provider = createOpenAICompatible({
        fetch: ollamaFetch,
        name: conn.providerId,
        baseURL: conn.baseUrl,
      });
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }

    case "apple_foundation":
      return createAppleFoundationModel(conn.modelId);

    default: {
      const config: Parameters<typeof createOpenAICompatible>[0] = {
        fetch: tauriFetch,
        name: conn.providerId,
        baseURL: conn.baseUrl,
      };
      if (conn.apiKey) {
        config.apiKey = conn.apiKey;
      }
      const provider = createOpenAICompatible(config);
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }
  }
};
