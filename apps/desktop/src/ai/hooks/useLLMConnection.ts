import { createAnthropic } from "@ai-sdk/anthropic";
import { createAzure } from "@ai-sdk/azure";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { extractReasoningMiddleware, wrapLanguageModel } from "ai";
import { useMemo } from "react";

import type { CharTask } from "@hypr/api-client";
import type { AIProviderStorage } from "@hypr/store";

import { createTracedFetch, tracedFetch } from "../traced-fetch";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { type ProviderId, PROVIDERS } from "~/settings/ai/llm/shared";
import { providerRowId } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  type ProviderEligibilityContext,
} from "~/settings/ai/shared/eligibility";
import * as settings from "~/store/tinybase/store/settings";

type LanguageModelV3 = Parameters<typeof wrapLanguageModel>[0]["model"];

type LLMConnectionInfo = {
  providerId: ProviderId;
  modelId: string;
  baseUrl: string;
  apiKey: string;
};

export type LLMConnectionStatus =
  | { status: "pending"; reason: "missing_provider" }
  | { status: "pending"; reason: "missing_model"; providerId: ProviderId }
  | { status: "error"; reason: "provider_not_found"; providerId: string }
  | { status: "error"; reason: "unauthenticated"; providerId: "hyprnote" }
  | { status: "error"; reason: "not_pro"; providerId: "hyprnote" }
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

export const useLanguageModel = (task?: CharTask): LanguageModelV3 | null => {
  const { conn } = useLLMConnection();
  return useMemo(
    () => (conn ? createLanguageModel(conn, task) : null),
    [conn, task],
  );
};

export const useLLMConnection = (): LLMConnectionResult => {
  const auth = useAuth();
  const billing = useBillingAccess();

  const { current_llm_provider, current_llm_model } = settings.UI.useValues(
    settings.STORE_ID,
  );
  const providerConfig = settings.UI.useRow(
    "ai_providers",
    current_llm_provider ? providerRowId("llm", current_llm_provider) : "",
    settings.STORE_ID,
  ) as AIProviderStorage | undefined;

  return useMemo<LLMConnectionResult>(
    () =>
      resolveLLMConnection({
        providerId: current_llm_provider,
        modelId: current_llm_model,
        providerConfig,
        session: auth?.session,
        isPro: billing.isPro,
      }),
    [
      auth,
      billing.isPro,
      current_llm_model,
      current_llm_provider,
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
  providerConfig: AIProviderStorage | undefined;
  session: { access_token: string } | null | undefined;
  isPro: boolean;
}): LLMConnectionResult => {
  const {
    providerId: rawProviderId,
    modelId,
    providerConfig,
    session,
    isPro,
  } = params;

  if (!rawProviderId) {
    return {
      conn: null,
      status: { status: "pending", reason: "missing_provider" },
    };
  }

  const providerId = rawProviderId as ProviderId;

  if (!modelId) {
    return {
      conn: null,
      status: { status: "pending", reason: "missing_model", providerId },
    };
  }

  const providerDefinition = PROVIDERS.find((p) => p.id === rawProviderId);

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
    isPro,
    config: { base_url: baseUrl, api_key: apiKey },
  };

  const blockers = getProviderSelectionBlockers(
    providerDefinition.requirements,
    context,
  );

  if (blockers.length > 0) {
    const blocker = blockers[0];
    if (blocker.code === "requires_auth" && providerId === "hyprnote") {
      return {
        conn: null,
        status: { status: "error", reason: "unauthenticated", providerId },
      };
    }
    if (blocker.code === "requires_entitlement" && providerId === "hyprnote") {
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

  if (providerId === "hyprnote" && session) {
    return {
      conn: {
        providerId,
        modelId,
        baseUrl: baseUrl ?? new URL("/llm", env.VITE_API_URL).toString(),
        apiKey: session.access_token,
      },
      status: { status: "success", providerId, isHosted: true },
    };
  }

  return {
    conn: { providerId, modelId, baseUrl, apiKey },
    status: { status: "success", providerId, isHosted: false },
  };
};

export const useFeedbackLanguageModel = (): LanguageModelV3 => {
  const { session } = useAuth();
  const apiKey = session?.access_token ?? "CANT_BE_EMPTY";

  return useMemo(() => {
    const baseUrl = new URL("/support/llm", env.VITE_API_URL).toString();
    const provider = createOpenRouter({
      fetch: tauriFetch,
      baseURL: baseUrl,
      apiKey,
    });
    return wrapWithThinkingMiddleware(provider.chat("unused"));
  }, [apiKey]);
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
): LanguageModelV3 => {
  switch (conn.providerId) {
    case "hyprnote": {
      const provider = createOpenRouter({
        fetch: task ? createTracedFetch(task) : tracedFetch,
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
