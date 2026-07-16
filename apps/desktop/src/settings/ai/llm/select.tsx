import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { cn } from "@hypr/utils";

import { useLlmSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import { getDefaultLlmSelection, getPreferredProviderModel } from "./selection";
import { type Provider, PROVIDERS } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { providerRowId, ProviderIconSlot } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { listAnthropicModels } from "~/settings/ai/shared/list-anthropic";
import { listAzureAIModels } from "~/settings/ai/shared/list-azure-ai";
import { listAzureOpenAIModels } from "~/settings/ai/shared/list-azure-openai";
import { listCloudflareWorkersAIModels } from "~/settings/ai/shared/list-cloudflare-workers-ai";
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
import { PersistAiSelection } from "~/settings/ai/shared/persist-selection";
import {
  getConfiguredProviderIds,
  getConfiguredProviders,
  getVisibleModelSelection,
} from "~/settings/ai/shared/selection";
import { useAiProvidersState } from "~/settings/providers";
import { useSetSettingValues } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { SettingsAlertToast } from "~/shared/ui/settings-alert";

export function SelectProviderAndModel() {
  const { t } = useLingui();
  const { providers: configuredProviders, isReady: providerSettingsReady } =
    useConfiguredMapping();
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const { setAccordionValue } = useLlmSettings();
  const [pendingProvider, setPendingProvider] = useState<string | null>(null);

  const { current_llm_model, current_llm_provider } = useConfigValues([
    "current_llm_model",
    "current_llm_provider",
  ] as const);
  const selectedProviderConfigured = current_llm_provider
    ? (configuredProviders[current_llm_provider]?.configured ?? false)
    : false;
  const visibleSelection = getVisibleModelSelection(
    current_llm_provider,
    current_llm_model,
    selectedProviderConfigured,
  );
  const providerOptions = getConfiguredProviders(
    PROVIDERS,
    configuredProviders,
  );
  const configuredProviderIds = getConfiguredProviderIds(
    PROVIDERS,
    configuredProviders,
    current_llm_provider,
  );

  const setSelection = useSetSettingValues();
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

  const needsDefaultSelection = !(
    visibleSelection.provider && visibleSelection.model
  );
  const defaultSelectionQuery = useQuery({
    queryKey: [
      "default-ai-selection",
      "llm",
      current_llm_provider ?? "",
      current_llm_model ?? "",
      configuredProviderIds,
    ],
    queryFn: async () =>
      await getDefaultLlmSelection(
        configuredProviderIds,
        current_llm_provider,
        current_llm_model,
        fetchModels,
      ),
    enabled:
      !pendingProvider &&
      providerSettingsReady &&
      needsDefaultSelection &&
      configuredProviderIds.length > 0,
    retry: false,
    staleTime: Infinity,
  });
  const defaultSelection = needsDefaultSelection
    ? defaultSelectionQuery.data
    : null;
  const effectiveSelection = pendingProvider
    ? { provider: pendingProvider, model: "" }
    : (defaultSelection ?? visibleSelection);

  const health = useConnectionHealth();
  const isConfigured = !!(
    effectiveSelection.provider && effectiveSelection.model
  );
  const hasError = isConfigured && health.status === "error";
  const alertDescription = !providerSettingsReady
    ? undefined
    : !isConfigured
      ? t`Language model is needed to make Anarlog summarize and chat about your conversations.`
      : hasError
        ? health.message
        : undefined;

  const handleProviderChange = (provider: string) => {
    if (provider === "hyprnote" && !billing.isPaid) {
      billing.upgradeToPro();
      return;
    }

    const requestId = ++selectionRequestRef.current;

    const status = configuredProviders[provider];
    if (!status?.listModels) {
      setAccordionValue(provider);
    }

    rememberModel(current_llm_provider, current_llm_model);
    setPendingProvider(provider);

    const nextModel = getPreferredProviderModel(
      lastSelectedModelsRef.current[provider],
      getCachedModels(provider),
      { allowSavedModelWithoutChoices: provider === "custom" },
    );

    if (nextModel) {
      setPendingProvider(null);
      rememberModel(provider, nextModel);
      setSelection({
        current_llm_provider: provider,
        current_llm_model: nextModel,
      });
      return;
    }

    void (async () => {
      let models: string[];
      try {
        models = await fetchModels(provider);
      } catch {
        return;
      }
      const resolvedModel = getPreferredProviderModel(
        lastSelectedModelsRef.current[provider],
        models,
        { allowSavedModelWithoutChoices: provider === "custom" },
      );

      if (selectionRequestRef.current !== requestId) {
        return;
      }

      if (!resolvedModel) {
        return;
      }

      setPendingProvider(null);
      rememberModel(provider, resolvedModel);
      setSelection({
        current_llm_provider: provider,
        current_llm_model: resolvedModel,
      });
    })();
  };

  const handleModelChange = (model: string) => {
    if (!effectiveSelection.provider) {
      return;
    }

    selectionRequestRef.current += 1;
    rememberModel(effectiveSelection.provider, model);
    setPendingProvider(null);
    setSelection({
      current_llm_provider: effectiveSelection.provider,
      current_llm_model: model,
    });
  };

  return (
    <div className="flex flex-col gap-4">
      {defaultSelection && !pendingProvider ? (
        <PersistAiSelection
          key={`llm:${defaultSelection.provider}:${defaultSelection.model}`}
          type="llm"
          provider={defaultSelection.provider}
          model={defaultSelection.model}
        />
      ) : null}
      <SettingsAlertToast
        id="llm-settings-alert"
        description={alertDescription}
        variant={hasError ? "error" : "warning"}
      />

      <h3 className="text-md font-sans font-semibold">
        <Trans>Model being used</Trans>
      </h3>
      <div className="flex flex-row items-center gap-4">
        <div className="min-w-0 flex-2" data-llm-provider-selector>
          <Select
            value={effectiveSelection.provider}
            onValueChange={handleProviderChange}
          >
            <SelectTrigger className="bg-card shadow-none focus:ring-0">
              <SelectValue placeholder={t`Select a provider`} />
            </SelectTrigger>
            <SelectContent>
              {providerOptions.map((provider) => {
                const requiresPro = requiresEntitlement(
                  provider.requirements,
                  "pro",
                );
                const locked = requiresPro && !billing.isPaid;
                const configured =
                  configuredProviders[provider.id]?.configured ?? false;

                return (
                  <SelectItem
                    key={provider.id}
                    value={provider.id}
                    disabled={locked || !configured}
                    className={cn([
                      "data-disabled:text-muted-foreground data-disabled:!opacity-100",
                      !configured && !locked && "text-muted-foreground",
                    ])}
                  >
                    <div className="flex flex-col gap-0.5">
                      <div className="flex items-center gap-2">
                        <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                        <span>{provider.displayName}</span>
                      </div>
                      {locked ? (
                        <span className="text-muted-foreground text-[11px]">
                          <Trans>Upgrade to Pro to use this provider.</Trans>
                        </span>
                      ) : null}
                    </div>
                  </SelectItem>
                );
              })}
            </SelectContent>
          </Select>
        </div>

        <span className="text-muted-foreground">/</span>

        <div className="min-w-0 flex-3">
          <ModelCombobox
            providerId={effectiveSelection.provider}
            value={effectiveSelection.model}
            onChange={handleModelChange}
            disabled={!effectiveSelection.provider}
            listModels={
              effectiveSelection.provider
                ? configuredProviders[effectiveSelection.provider]?.listModels
                : undefined
            }
            isConfigured={isConfigured && health.status === "success"}
            suffix={isConfigured ? <HealthStatusIndicator /> : undefined}
          />
        </div>
      </div>
    </div>
  );
}

type ProviderStatus = {
  configured: boolean;
  listModels?: () => Promise<ListModelsResult>;
};

type ProviderConfig = {
  base_url?: unknown;
  api_key?: unknown;
};

export function getLlmProviderStatus({
  provider,
  config,
  isAuthenticated,
  isPaid,
}: {
  provider: Provider;
  config?: ProviderConfig;
  isAuthenticated: boolean;
  isPaid: boolean;
}): ProviderStatus {
  const baseUrl = String(config?.base_url || provider.baseUrl || "").trim();
  const apiKey = String(config?.api_key || "").trim();

  const eligible =
    getProviderSelectionBlockers(provider.requirements, {
      isAuthenticated,
      isPaid,
      config: { base_url: baseUrl, api_key: apiKey },
    }).length === 0;

  if (!eligible) {
    return { configured: false };
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
    return { configured: true, listModels: async () => result };
  }

  let listModelsFunc: () => Promise<ListModelsResult>;

  switch (provider.id) {
    case "openai":
      listModelsFunc = () => listOpenAIModels(baseUrl, apiKey);
      break;
    case "cloudflare_workers_ai":
      listModelsFunc = () => listCloudflareWorkersAIModels(baseUrl, apiKey);
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

  return { configured: true, listModels: listModelsFunc };
}

function useConfiguredMapping(): {
  providers: Record<string, ProviderStatus>;
  isReady: boolean;
} {
  const auth = useAuth();
  const billing = useBillingAccess();
  const { providers: configuredProviders, isReady } =
    useAiProvidersState("llm");

  const mapping = useMemo(() => {
    return Object.fromEntries(
      PROVIDERS.map((provider) => {
        const config = configuredProviders[providerRowId("llm", provider.id)];
        return [
          provider.id,
          getLlmProviderStatus({
            provider,
            config,
            isAuthenticated: !!auth?.session,
            isPaid: billing.isPaid,
          }),
        ];
      }),
    ) as Record<string, ProviderStatus>;
  }, [configuredProviders, auth, billing]);

  return { providers: mapping, isReady };
}
