import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { cn } from "@anlg/utils";

import { useConfiguredLlmProviders } from "./configured-providers";
import { useLlmSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import {
  getDefaultLlmSelection,
  getPreferredProviderModel,
  isSameModelSelection,
  shouldShowMissingModelWarning,
} from "./selection";
import { PROVIDERS } from "./shared";

import { modelMetadataQueryOptions } from "~/ai/hooks/useModelMetadata";
import {
  normalizeReasoningEffort,
  supportsReasoningEffort,
} from "~/ai/reasoning-effort";
import { useBillingAccess } from "~/auth/billing-context";
import { ProviderIconSlot } from "~/settings/ai/shared";
import { requiresEntitlement } from "~/settings/ai/shared/eligibility";
import { ModelCombobox } from "~/settings/ai/shared/model-combobox";
import { PersistAiSelection } from "~/settings/ai/shared/persist-selection";
import {
  getConfiguredProviderIds,
  getConfiguredProviders,
  getVisibleModelSelection,
} from "~/settings/ai/shared/selection";
import {
  setSettingValues,
  useSetSettingValue,
  useSettingsReady,
} from "~/settings/queries";
import { SETTING_CONTROL_CLASS, SettingRow } from "~/settings/setting-row";
import { useConfigValues } from "~/shared/config";
import { SettingsAlertToast } from "~/shared/ui/settings-alert";

export function SelectProviderAndModel() {
  const { t } = useLingui();
  const { providers: configuredProviders, isReady: providerSettingsReady } =
    useConfiguredLlmProviders();
  const intelligenceDisabled = useConfigValues([
    "intelligence_disabled",
  ] as const).intelligence_disabled;
  const settingsReady = useSettingsReady() && !intelligenceDisabled;
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const { setAccordionValue } = useLlmSettings();
  const [pendingSelection, setPendingSelection] = useState<{
    provider: string;
    model: string;
    originProvider: string | undefined;
    originModel: string | undefined;
  } | null>(null);
  const [isResolvingProvider, setIsResolvingProvider] = useState(false);

  const {
    current_llm_model,
    current_llm_provider,
    current_llm_reasoning_effort,
  } = useConfigValues([
    "current_llm_model",
    "current_llm_provider",
    "current_llm_reasoning_effort",
  ] as const);
  const setReasoningEffort = useSetSettingValue("current_llm_reasoning_effort");
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
  const pendingSelectionSettled =
    pendingSelection &&
    isSameModelSelection(
      current_llm_provider,
      current_llm_model,
      pendingSelection.provider,
      pendingSelection.model,
    );
  if (pendingSelectionSettled) {
    setPendingSelection(null);
  }
  const activePendingSelection =
    pendingSelection &&
    !pendingSelectionSettled &&
    isSameModelSelection(
      current_llm_provider,
      current_llm_model,
      pendingSelection.originProvider,
      pendingSelection.originModel,
    )
      ? pendingSelection
      : null;

  const lastSelectedModelsRef = useRef<Record<string, string>>(
    current_llm_provider && current_llm_model
      ? { [current_llm_provider]: current_llm_model }
      : {},
  );
  const selectionRequestRef = useRef(0);

  const persistSelection = (
    provider: string,
    model: string,
    requestId: number,
  ) => {
    // Effort support differs per model, so a stale level would break
    // requests against a model that rejects the parameter.
    void setSettingValues({
      current_llm_provider: provider,
      current_llm_model: model,
      current_llm_reasoning_effort: "default",
    }).catch((error) => {
      console.error("[settings] failed to update LLM selection", error);
      if (selectionRequestRef.current === requestId) {
        setPendingSelection(null);
      }
    });
  };

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
      queryClient.getQueryData(
        modelMetadataQueryOptions(
          provider,
          status.listModels,
          status.modelScope,
        ).queryKey,
      )?.models ?? []
    );
  };

  const fetchModels = async (provider: string) => {
    const status = configuredProviders[provider];
    const listModels = status?.listModels;
    if (!listModels) {
      return [];
    }

    const result = await queryClient.fetchQuery(
      modelMetadataQueryOptions(provider, listModels, status.modelScope),
    );

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
      !activePendingSelection &&
      providerSettingsReady &&
      needsDefaultSelection &&
      configuredProviderIds.length > 0,
    retry: false,
    staleTime: Infinity,
  });
  const defaultSelection = needsDefaultSelection
    ? defaultSelectionQuery.data
    : null;
  const effectiveSelection = activePendingSelection
    ? {
        provider: activePendingSelection.provider,
        model: activePendingSelection.model,
      }
    : (defaultSelection ?? visibleSelection);

  const health = useConnectionHealth();
  const isConfigured = !!(
    effectiveSelection.provider && effectiveSelection.model
  );
  const hasError =
    isConfigured && !activePendingSelection && health.status === "error";
  const isResolvingSelection =
    isResolvingProvider || defaultSelectionQuery.isFetching;
  const showMissingModelWarning = shouldShowMissingModelWarning({
    isConfigured,
    isResolvingSelection,
    providerSettingsReady,
    settingsReady,
  });
  const alertDescription = showMissingModelWarning
    ? t`Choose a language model for summaries and chat.`
    : providerSettingsReady &&
        settingsReady &&
        !isResolvingSelection &&
        hasError
      ? health.message
      : undefined;

  const handleProviderChange = (provider: string) => {
    if (provider === "anarlog" && !billing.isPaid) {
      billing.upgradeToPro();
      return;
    }

    const requestId = ++selectionRequestRef.current;

    const status = configuredProviders[provider];
    if (!status?.listModels) {
      setAccordionValue(provider);
    }

    rememberModel(current_llm_provider, current_llm_model);
    const originSelection = {
      originProvider: current_llm_provider,
      originModel: current_llm_model,
    };
    setPendingSelection({ provider, model: "", ...originSelection });
    setIsResolvingProvider(false);

    const nextModel = getPreferredProviderModel(
      lastSelectedModelsRef.current[provider],
      getCachedModels(provider),
      { allowSavedModelWithoutChoices: provider === "custom" },
    );

    if (nextModel) {
      setPendingSelection({ provider, model: nextModel, ...originSelection });
      rememberModel(provider, nextModel);
      persistSelection(provider, nextModel, requestId);
      return;
    }

    setIsResolvingProvider(true);
    void (async () => {
      let models: string[];
      try {
        models = await fetchModels(provider);
      } catch {
        if (selectionRequestRef.current === requestId) {
          setIsResolvingProvider(false);
          if (provider !== "custom") {
            setPendingSelection(null);
          }
        }
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

      setIsResolvingProvider(false);
      if (!resolvedModel) {
        if (provider !== "custom") {
          setPendingSelection(null);
        }
        return;
      }

      setPendingSelection({
        provider,
        model: resolvedModel,
        ...originSelection,
      });
      rememberModel(provider, resolvedModel);
      persistSelection(provider, resolvedModel, requestId);
    })();
  };

  const handleModelChange = (model: string) => {
    if (!effectiveSelection.provider) {
      return;
    }

    const requestId = ++selectionRequestRef.current;
    rememberModel(effectiveSelection.provider, model);
    setPendingSelection({
      provider: effectiveSelection.provider,
      model,
      originProvider: current_llm_provider,
      originModel: current_llm_model,
    });
    setIsResolvingProvider(false);
    persistSelection(effectiveSelection.provider, model, requestId);
  };

  return (
    <div className="flex flex-col gap-4">
      {defaultSelection && !activePendingSelection ? (
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
        lifecycle="condition-bound"
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
            <SelectTrigger className="bg-card shadow-none">
              <SelectValue placeholder={t`Select a provider`} />
            </SelectTrigger>
            <SelectContent>
              {providerOptions.length === 0 && (
                <div className="text-muted-foreground px-2 py-3 text-center text-sm">
                  <Trans>No providers found.</Trans>
                </div>
              )}
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
            modelScope={
              configuredProviders[effectiveSelection.provider]?.modelScope
            }
            isConfigured={isConfigured && health.status === "success"}
            suffix={isConfigured ? <HealthStatusIndicator /> : undefined}
          />
        </div>
      </div>

      {effectiveSelection.provider &&
      supportsReasoningEffort(effectiveSelection.provider) ? (
        <SettingRow
          title={<Trans>Reasoning effort</Trans>}
          description={
            <Trans>
              How much the model thinks before answering. Default leaves it to
              the provider.
            </Trans>
          }
        >
          {(labelProps) => (
            <Select
              value={normalizeReasoningEffort(current_llm_reasoning_effort)}
              onValueChange={(value) =>
                setReasoningEffort(normalizeReasoningEffort(value))
              }
            >
              <SelectTrigger {...labelProps} className={SETTING_CONTROL_CLASS}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="default">
                  <Trans>Default</Trans>
                </SelectItem>
                <SelectItem value="low">
                  <Trans>Low</Trans>
                </SelectItem>
                <SelectItem value="medium">
                  <Trans>Medium</Trans>
                </SelectItem>
                <SelectItem value="high">
                  <Trans>High</Trans>
                </SelectItem>
              </SelectContent>
            </Select>
          )}
        </SettingRow>
      ) : null}
    </div>
  );
}
