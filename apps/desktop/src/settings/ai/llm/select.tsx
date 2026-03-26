import { useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { cn } from "@hypr/utils";

import { HealthStatusIndicator, useConnectionHealth } from "./health";
import { getPreferredProviderModel } from "./selection";
import { PROVIDERS } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { providerRowId } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { listAnthropicModels } from "~/settings/ai/shared/list-anthropic";
import { listAzureAIModels } from "~/settings/ai/shared/list-azure-ai";
import { listAzureOpenAIModels } from "~/settings/ai/shared/list-azure-openai";
import {
  type InputModality,
  type ListModelsResult,
} from "~/settings/ai/shared/list-common";
import { listGoogleModels } from "~/settings/ai/shared/list-google";
import { listLMStudioModels } from "~/settings/ai/shared/list-lmstudio";
import { listMistralModels } from "~/settings/ai/shared/list-mistral";
import { listOllamaModels } from "~/settings/ai/shared/list-ollama";
import {
  listGenericModels,
  listOpenAIModels,
} from "~/settings/ai/shared/list-openai";
import { listOpenRouterModels } from "~/settings/ai/shared/list-openrouter";
import { ModelCombobox } from "~/settings/ai/shared/model-combobox";
import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

export function SelectProviderAndModel() {
  const configuredProviders = useConfiguredMapping();
  const billing = useBillingAccess();
  const queryClient = useQueryClient();

  const { current_llm_model, current_llm_provider } = useConfigValues([
    "current_llm_model",
    "current_llm_provider",
  ] as const);

  const health = useConnectionHealth();
  const isConfigured = !!(current_llm_provider && current_llm_model);
  const hasError = isConfigured && health.status === "error";

  const handleSelectProvider = settings.UI.useSetValueCallback(
    "current_llm_provider",
    (provider: string) => provider,
    [],
    settings.STORE_ID,
  );
  const handleSelectModel = settings.UI.useSetValueCallback(
    "current_llm_model",
    (model: string) => model,
    [],
    settings.STORE_ID,
  );
  const lastSelectedModelsRef = useRef<Record<string, string>>(
    current_llm_provider && current_llm_model
      ? { [current_llm_provider]: current_llm_model }
      : {},
  );
  const selectionRequestRef = useRef(0);

  const rememberModel = (provider?: string, model?: string) => {
    if (!provider || model === undefined) {
      return;
    }

    lastSelectedModelsRef.current[provider] = model;
  };

  const getCachedModels = (provider: string) => {
    const status = configuredProviders[provider];
    if (!status?.listModels) {
      return [];
    }

    return (
      queryClient.getQueryData<ListModelsResult>([
        "models",
        provider,
        status.listModels,
      ])?.models ?? []
    );
  };

  const fetchModels = async (provider: string) => {
    const status = configuredProviders[provider];
    const listModels = status?.listModels;
    if (!listModels) {
      return [];
    }

    const result = await queryClient.fetchQuery({
      queryKey: ["models", provider, listModels],
      queryFn: async () => await listModels(),
      retry: 3,
      retryDelay: 300,
      staleTime: 1000 * 2,
    });

    return result.models;
  };

  const handleProviderChange = (provider: string) => {
    if (provider === "hyprnote" && !billing.isPro) {
      billing.upgradeToPro();
      return;
    }

    rememberModel(current_llm_provider, current_llm_model);

    const nextModel = getPreferredProviderModel(
      lastSelectedModelsRef.current[provider],
      getCachedModels(provider),
      { allowSavedModelWithoutChoices: provider === "custom" },
    );

    rememberModel(provider, nextModel);
    handleSelectProvider(provider);
    handleSelectModel(nextModel);

    const requestId = ++selectionRequestRef.current;
    void (async () => {
      const models = await fetchModels(provider);
      const resolvedModel = getPreferredProviderModel(
        lastSelectedModelsRef.current[provider],
        models,
        { allowSavedModelWithoutChoices: provider === "custom" },
      );

      if (selectionRequestRef.current !== requestId) {
        return;
      }

      rememberModel(provider, resolvedModel);
      handleSelectModel(resolvedModel);
    })();
  };

  const handleModelChange = (model: string) => {
    if (!current_llm_provider) {
      return;
    }

    rememberModel(current_llm_provider, model);
    handleSelectModel(model);
  };

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-md font-serif font-semibold">Model being used</h3>
      <div
        className={cn([
          "flex flex-col gap-4",
          "rounded-xl border border-neutral-200 p-4",
          !isConfigured || hasError ? "bg-red-50" : "bg-neutral-50",
        ])}
      >
        <div className="flex flex-row items-center gap-4">
          <div className="min-w-0 flex-2" data-llm-provider-selector>
            <Select
              value={current_llm_provider || ""}
              onValueChange={handleProviderChange}
            >
              <SelectTrigger className="bg-white shadow-none focus:ring-0">
                <SelectValue placeholder="Select a provider" />
              </SelectTrigger>
              <SelectContent>
                {PROVIDERS.map((provider) => {
                  const status = configuredProviders[provider.id];
                  const requiresPro = requiresEntitlement(
                    provider.requirements,
                    "pro",
                  );
                  const locked = requiresPro && !billing.isPro;

                  return (
                    <SelectItem
                      key={provider.id}
                      value={provider.id}
                      disabled={!status?.listModels || locked}
                    >
                      <div className="flex flex-col gap-0.5">
                        <div className="flex items-center gap-2">
                          {provider.icon}
                          <span>{provider.displayName}</span>
                          {requiresPro ? (
                            <span className="rounded-full border border-neutral-200 px-2 py-0.5 text-[10px] tracking-wide text-neutral-500 uppercase">
                              Pro
                            </span>
                          ) : null}
                        </div>
                        {locked ? (
                          <span className="text-[11px] text-neutral-500">
                            Upgrade to Pro to use this provider.
                          </span>
                        ) : null}
                      </div>
                    </SelectItem>
                  );
                })}
              </SelectContent>
            </Select>
          </div>

          <span className="text-neutral-500">/</span>

          <div className="min-w-0 flex-3">
            <ModelCombobox
              providerId={current_llm_provider || ""}
              value={current_llm_model || ""}
              onChange={handleModelChange}
              disabled={!current_llm_provider}
              listModels={
                current_llm_provider
                  ? configuredProviders[current_llm_provider]?.listModels
                  : undefined
              }
              isConfigured={isConfigured}
              suffix={isConfigured ? <HealthStatusIndicator /> : undefined}
            />
          </div>
        </div>

        {!isConfigured && (
          <div className="flex items-center gap-2 border-t border-red-200 pt-2">
            <span className="text-sm text-red-600">
              <strong className="font-medium">Language model</strong> is needed
              to make Char summarize and chat about your conversations.
            </span>
          </div>
        )}

        {hasError && health.message && (
          <div className="flex items-center gap-2 border-t border-red-200 pt-2">
            <span className="text-sm text-red-600">{health.message}</span>
          </div>
        )}
      </div>
    </div>
  );
}

type ProviderStatus = {
  listModels?: () => Promise<ListModelsResult>;
};

function useConfiguredMapping(): Record<string, ProviderStatus> {
  const auth = useAuth();
  const billing = useBillingAccess();
  const configuredProviders = settings.UI.useResultTable(
    settings.QUERIES.llmProviders,
    settings.STORE_ID,
  );

  const mapping = useMemo(() => {
    return Object.fromEntries(
      PROVIDERS.map((provider) => {
        const config = configuredProviders[providerRowId("llm", provider.id)];
        const baseUrl = String(
          config?.base_url || provider.baseUrl || "",
        ).trim();
        const apiKey = String(config?.api_key || "").trim();

        const eligible =
          getProviderSelectionBlockers(provider.requirements, {
            isAuthenticated: !!auth?.session,
            isPro: billing.isPro,
            config: { base_url: baseUrl, api_key: apiKey },
          }).length === 0;

        if (!eligible) {
          return [provider.id, { listModels: undefined }];
        }

        if (provider.id === "hyprnote") {
          const result: ListModelsResult = {
            models: ["Auto"],
            ignored: [],
            metadata: {
              Auto: {
                input_modalities: ["text", "image"] as InputModality[],
              },
            },
          };
          return [provider.id, { listModels: async () => result }];
        }

        let listModelsFunc: () => Promise<ListModelsResult>;

        switch (provider.id) {
          case "openai":
            listModelsFunc = () => listOpenAIModels(baseUrl, apiKey);
            break;
          case "anthropic":
            listModelsFunc = () => listAnthropicModels(baseUrl, apiKey);
            break;
          case "openrouter":
            listModelsFunc = () => listOpenRouterModels(baseUrl, apiKey);
            break;
          case "google_generative_ai":
            listModelsFunc = () => listGoogleModels(baseUrl, apiKey);
            break;
          case "mistral":
            listModelsFunc = () => listMistralModels(baseUrl, apiKey);
            break;
          case "azure_openai":
            listModelsFunc = () => listAzureOpenAIModels(baseUrl, apiKey);
            break;
          case "azure_ai":
            listModelsFunc = () => listAzureAIModels(baseUrl, apiKey);
            break;
          case "ollama":
            listModelsFunc = () => listOllamaModels(baseUrl, apiKey);
            break;
          case "lmstudio":
            listModelsFunc = () => listLMStudioModels(baseUrl, apiKey);
            break;
          case "custom":
            listModelsFunc = () => listGenericModels(baseUrl, apiKey);
            break;
          default:
            listModelsFunc = () => listGenericModels(baseUrl, apiKey);
        }

        return [provider.id, { listModels: listModelsFunc }];
      }),
    ) as Record<string, ProviderStatus>;
  }, [configuredProviders, auth, billing]);

  return mapping;
}
