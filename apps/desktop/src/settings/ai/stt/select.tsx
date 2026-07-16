import { Trans, useLingui } from "@lingui/react/macro";
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { arch } from "@tauri-apps/plugin-os";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  Loader2,
  Trash2,
} from "lucide-react";
import { useRef, useState } from "react";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import type { AIProviderStorage } from "@hypr/store";
import { Input } from "@hypr/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useSttSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import { LocalModelBackendBadge, LocalModelLabel } from "./model-icon";
import {
  getDefaultSttSelection,
  getLanguageSupportIssue,
  resolveLiveLanguageSupportMode,
} from "./selection";
import {
  displayModelLabel,
  displayModelTitle,
  formatModelSize,
  type ProviderId,
  PROVIDERS,
  sttModelQueries,
} from "./shared";

import { useBillingAccess } from "~/auth/billing-context";
import { useNotifications } from "~/contexts/notifications";
import { providerRowId, ProviderIconSlot } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { PersistAiSelection } from "~/settings/ai/shared/persist-selection";
import {
  getConfiguredProviderIds,
  getConfiguredProviders,
  getVisibleModelSelection,
} from "~/settings/ai/shared/selection";
import { getBaseLanguageDisplayName } from "~/settings/general/language";
import { useAiProvidersState } from "~/settings/providers";
import { useSetSettingValues } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { SettingsAlertToast } from "~/shared/ui/settings-alert";
import {
  isConfiguredSttModel,
  isHyprnoteLocalSttModel,
  isLiveTranscriptionSupported,
  isRealtimeLocalModel,
  isSupportedLanguagesBatch,
  isSupportedLanguagesLive,
  isSupportedLocalSttModel,
} from "~/stt/capabilities";
import {
  getDefaultSttModel,
  getPreferredProviderModel,
} from "~/stt/model-selection";

export function SelectProviderAndModel() {
  const { t } = useLingui();
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const billing = useBillingAccess();
  const { providers: configuredProviders, isReady: providerSettingsReady } =
    useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const health = useConnectionHealth();
  const [pendingProvider, setPendingProvider] = useState<ProviderId | null>(
    null,
  );

  const selectedSttModel = isConfiguredSttModel(
    current_stt_provider,
    current_stt_model,
  )
    ? current_stt_model
    : undefined;
  const selectedProvider = current_stt_provider as ProviderId | undefined;
  const selectedProviderConfigured = selectedProvider
    ? (configuredProviders[selectedProvider]?.configured ?? false)
    : false;
  const visibleSelection = getVisibleModelSelection(
    selectedProvider,
    selectedSttModel,
    selectedProviderConfigured,
  );
  const selectableProviders = PROVIDERS.filter(({ disabled }) => !disabled);
  const configuredProviderIds = getConfiguredProviderIds(
    selectableProviders,
    configuredProviders,
    selectedProvider,
  );
  const defaultSelection =
    providerSettingsReady && !visibleSelection.model
      ? getDefaultSttSelection(
          configuredProviderIds,
          configuredProviders,
          selectedProvider,
          current_stt_model,
        )
      : null;
  const effectiveSelection = pendingProvider
    ? { provider: pendingProvider, model: "" }
    : (defaultSelection ?? visibleSelection);
  const visibleProvider = effectiveSelection.provider as ProviderId | "";
  const isConfigured = !!(visibleProvider && effectiveSelection.model);
  const hasError = isConfigured && health.status === "error";
  const alertDescription = !providerSettingsReady
    ? undefined
    : !isConfigured
      ? t`Transcription model is needed to make Anarlog listen to your conversations.`
      : hasError
        ? health.message
        : undefined;
  const selectedModels = visibleProvider
    ? (configuredProviders[visibleProvider]?.models ?? [])
    : [];
  const displayedSttModel =
    visibleProvider === "custom"
      ? effectiveSelection.model
      : effectiveSelection.model
        ? getPreferredProviderModel(effectiveSelection.model, selectedModels, {
            keepUnavailableSavedModel: true,
          })
        : undefined;
  const selectedModel = selectedModels.find(
    (model) => model.id === displayedSttModel,
  );
  const providerOptions = getConfiguredProviders(
    selectableProviders,
    configuredProviders,
  );

  const setSelection = useSetSettingValues();
  const lastSelectedModelsRef = useRef<Record<string, string>>(
    current_stt_provider && selectedSttModel
      ? { [current_stt_provider]: selectedSttModel }
      : {},
  );
  const rememberModel = (provider?: string, model?: string) => {
    if (!provider || model === undefined) {
      return;
    }

    lastSelectedModelsRef.current[provider] = model;
  };

  const handleProviderChange = (provider: string) => {
    rememberModel(current_stt_provider, selectedSttModel);

    const providerId = provider as ProviderId;
    const nextModels = configuredProviders[providerId]?.models ?? [];
    const nextModel =
      getPreferredProviderModel(
        lastSelectedModelsRef.current[provider],
        nextModels,
        { allowSavedModelWithoutChoices: providerId === "custom" },
      ) ||
      getDefaultSttModel(providerId) ||
      "";

    if (!nextModel) {
      setPendingProvider(providerId);
      return;
    }

    setPendingProvider(null);
    rememberModel(provider, nextModel);
    setSelection({
      current_stt_provider: provider,
      current_stt_model: nextModel,
    });
  };

  const handleModelChange = (model: string) => {
    if (!visibleProvider) {
      return;
    }

    rememberModel(visibleProvider, model);
    setPendingProvider(null);
    setSelection({
      current_stt_provider: visibleProvider,
      current_stt_model: model,
    });
  };
  return (
    <div className="flex flex-col gap-4">
      {defaultSelection && !pendingProvider ? (
        <PersistAiSelection
          key={`stt:${defaultSelection.provider}:${defaultSelection.model}`}
          type="stt"
          provider={defaultSelection.provider}
          model={defaultSelection.model}
        />
      ) : null}
      <SettingsAlertToast
        id="stt-settings-alert"
        description={alertDescription}
        variant={hasError ? "error" : "warning"}
      />
      {!alertDescription && <TranscriptionLanguageWarningToast />}

      <h3 className="text-md font-sans font-semibold">
        <Trans>Model being used</Trans>
      </h3>
      <div className="flex flex-row items-center gap-4">
        <div className="min-w-0 flex-2" data-stt-provider-selector>
          <Select value={visibleProvider} onValueChange={handleProviderChange}>
            <SelectTrigger className="bg-card shadow-none focus:ring-0">
              <SelectValue placeholder={t`Select a provider`} />
            </SelectTrigger>
            <SelectContent>
              {providerOptions.map((provider) => {
                const configured =
                  configuredProviders[provider.id]?.configured ?? false;
                const requiresPro = requiresEntitlement(
                  provider.requirements,
                  "pro",
                );
                const locked = requiresPro && !billing.isPaid;
                return (
                  <SelectItem
                    key={provider.id}
                    value={provider.id}
                    disabled={provider.disabled || locked}
                    className={cn([
                      "data-disabled:text-muted-foreground data-disabled:!opacity-100",
                      !configured && !locked && "text-muted-foreground",
                    ])}
                  >
                    <div className="flex flex-col gap-0.5">
                      <div className="flex items-center gap-2">
                        <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                        <span>{provider.displayName}</span>
                        {requiresPro ? (
                          <span className="border-border text-muted-foreground rounded-full border px-2 py-0.5 text-[10px] tracking-wide uppercase">
                            <Trans>Pro</Trans>
                          </span>
                        ) : null}
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

        {visibleProvider === "custom" ? (
          <div className="min-w-0 flex-3">
            <Input
              value={displayedSttModel || ""}
              onChange={(event) => handleModelChange(event.target.value)}
              className="text-xs"
              placeholder={t`Enter a model identifier`}
            />
          </div>
        ) : (
          <div className="min-w-0 flex-3">
            <Select
              value={displayedSttModel || ""}
              onValueChange={handleModelChange}
              disabled={selectedModels.length === 0}
            >
              <SelectTrigger
                className={cn([
                  "bg-card text-left shadow-none focus:ring-0",
                  "[&>span]:!flex [&>span]:w-full [&>span]:min-w-0 [&>span]:items-center [&>span]:justify-start [&>span]:gap-2 [&>span]:overflow-visible [&>span]:[-webkit-line-clamp:unset]",
                  isConfigured && "[&>svg:last-child]:hidden",
                ])}
              >
                <SelectValue placeholder={t`Select a model`}>
                  {selectedModel ? (
                    <ModelSelectedValue model={selectedModel} />
                  ) : undefined}
                </SelectValue>
                {isConfigured && <HealthStatusIndicator />}
                {isConfigured && health.status === "success" && (
                  <Check className="-mr-1 h-4 w-4 shrink-0 text-green-600" />
                )}
              </SelectTrigger>
              <SelectContent align="end">
                {selectedModels.map((model, i) => {
                  const prevCategory =
                    i > 0 ? selectedModels[i - 1].category : null;
                  const showHeader =
                    model.category && model.category !== prevCategory;
                  const categoryLabel = showHeader
                    ? getModelCategoryLabel(model.category)
                    : null;
                  return (
                    <span key={model.id}>
                      {categoryLabel && (
                        <div className="text-muted-foreground px-2 pt-2 pb-1 text-[11px] font-medium tracking-wide uppercase">
                          {categoryLabel}
                        </div>
                      )}
                      <ModelSelectItem
                        model={model}
                        onDownload={() => startDownload(model.id as LocalModel)}
                        onStartTrial={startTrial}
                      />
                    </span>
                  );
                })}
              </SelectContent>
            </Select>
          </div>
        )}
      </div>
    </div>
  );
}

const TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID =
  "transcription-language-warning";
const dismissedTranscriptionLanguageWarningKeys = new Set<string>();

function TranscriptionLanguageWarningToast() {
  const { i18n, t } = useLingui();
  const warning = useTranscriptionLanguageWarning();

  if (!warning || dismissedTranscriptionLanguageWarningKeys.has(warning.key)) {
    return null;
  }

  const model = displayModelLabel(warning.model);
  const unsupportedLanguages = warning.unsupportedLanguages.map((language) =>
    getBaseLanguageDisplayName(language, i18n.locale),
  );
  const description =
    unsupportedLanguages.length > 0
      ? t`${model} can't transcribe ${formatLanguageList(unsupportedLanguages)}. Try another model or change your spoken languages.`
      : t`${model} can't transcribe all selected languages together. Try another model or use fewer spoken languages.`;

  return (
    <TranscriptionLanguageWarningToastLifecycle
      key={warning.key}
      warningKey={warning.key}
      description={description}
      actionLabel={t`Got it`}
    />
  );
}

function TranscriptionLanguageWarningToastLifecycle({
  warningKey,
  description,
  actionLabel,
}: {
  warningKey: string;
  description: string;
  actionLabel: string;
}) {
  useMountEffect(() => {
    sonnerToast.warning(description, {
      id: TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID,
      duration: Infinity,
      icon: <AlertTriangle className="size-4 shrink-0 text-amber-500" />,
      action: {
        label: actionLabel,
        onClick: () => {
          dismissedTranscriptionLanguageWarningKeys.add(warningKey);
          clearTranscriptionLanguageWarningToast();
        },
      },
    });

    return clearTranscriptionLanguageWarningToast;
  });

  return null;
}

function clearTranscriptionLanguageWarningToast() {
  sonnerToast.dismiss(TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID);
}

function useTranscriptionLanguageWarning() {
  const { current_stt_provider, current_stt_model, spoken_languages } =
    useConfigValues([
      "current_stt_provider",
      "current_stt_model",
      "spoken_languages",
    ] as const);
  const health = useConnectionHealth();

  const selectedSttModel = isConfiguredSttModel(
    current_stt_provider,
    current_stt_model,
  )
    ? current_stt_model
    : undefined;
  const isConfigured = !!(current_stt_provider && selectedSttModel);
  const isOnDeviceModel = isHyprnoteLocalSttModel(
    current_stt_provider,
    selectedSttModel,
  );
  const useLiveOnDeviceModel =
    isOnDeviceModel && isRealtimeLocalModel(selectedSttModel);
  const hasError = isConfigured && health.status === "error";
  const liveSupport = useQuery({
    queryKey: ["stt-live-support", current_stt_provider, selectedSttModel],
    queryFn: () =>
      isLiveTranscriptionSupported(current_stt_provider, selectedSttModel),
    enabled: isConfigured,
  });
  const useLiveMode = resolveLiveLanguageSupportMode({
    isOnDeviceModel,
    useLiveOnDeviceModel,
    liveSupported: liveSupport.data,
  });

  const languageSupportIssue = useQuery({
    queryKey: [
      "stt-language-support",
      current_stt_provider,
      selectedSttModel,
      useLiveMode,
      spoken_languages,
    ],
    queryFn: async () => {
      const isSupported = (languages: readonly string[]) =>
        useLiveMode
          ? isSupportedLanguagesLive(
              current_stt_provider!,
              selectedSttModel ?? null,
              languages,
            )
          : isSupportedLanguagesBatch(
              current_stt_provider!,
              selectedSttModel ?? null,
              languages,
            );

      return await getLanguageSupportIssue(spoken_languages ?? [], isSupported);
    },
    enabled:
      isConfigured &&
      liveSupport.data !== undefined &&
      !!spoken_languages?.length,
  });

  if (
    !isConfigured ||
    !selectedSttModel ||
    !languageSupportIssue.data ||
    hasError
  ) {
    return null;
  }

  return {
    key: [
      current_stt_provider,
      selectedSttModel,
      ...(spoken_languages ?? []),
    ].join(":"),
    model: selectedSttModel,
    unsupportedLanguages: languageSupportIssue.data.unsupportedLanguages,
  };
}

function formatLanguageList(languages: string[]) {
  const visibleLanguages = languages.slice(0, 3);
  const remainingCount = languages.length - visibleLanguages.length;

  if (remainingCount > 0) {
    visibleLanguages.push(`${remainingCount} more`);
  }

  return visibleLanguages.join(", ");
}

type ModelCategory = "latest" | null;
type ModelEntry = {
  id: string;
  isDownloaded: boolean;
  displayName?: string;
  isDeprecated?: boolean;
  category?: ModelCategory;
  sizeBytes?: number | null;
  mode?: "realtime" | "batch";
};

function getModelCategoryLabel(category?: ModelCategory) {
  if (category === "latest") {
    return "Recommended";
  }

  return null;
}

function getProviderModelMode(
  providerId: ProviderId,
  model: string,
): ModelEntry["mode"] {
  if (providerId === "assemblyai") {
    if (model === "universal-3-pro") {
      return "batch";
    }

    if (model === "u3-rt-pro") {
      return "realtime";
    }
  }

  if (providerId === "elevenlabs") {
    if (model === "scribe_v2") {
      return "batch";
    }

    if (model === "scribe_v2_realtime") {
      return "realtime";
    }
  }

  if (providerId === "mistral") {
    if (model === "voxtral-mini-2602" || model === "voxtral-mini-latest") {
      return "batch";
    }

    if (model === "voxtral-mini-transcribe-realtime-2602") {
      return "realtime";
    }
  }

  if (providerId === "soniox") {
    if (model === "stt-async-v5" || model === "stt-async-v4") {
      return "batch";
    }

    if (
      model === "stt-rt-v5" ||
      model === "stt-rt-v4" ||
      model === "stt-v5" ||
      model === "stt-v4"
    ) {
      return "realtime";
    }
  }

  return undefined;
}

function useConfiguredMapping(): {
  providers: Record<
    ProviderId,
    {
      configured: boolean;
      models: ModelEntry[];
    }
  >;
  isReady: boolean;
} {
  const billing = useBillingAccess();
  const { providers: configuredProviders, isReady } =
    useAiProvidersState("stt");

  const targetArch = useQuery({
    queryKey: ["target-arch"],
    queryFn: () => arch(),
    staleTime: Infinity,
  });

  const isAppleSilicon = targetArch.data === "aarch64";

  const supportedModels = useQuery({
    queryKey: ["list-supported-models"],
    queryFn: async () => {
      const result = await localSttCommands.listSupportedModels();
      return result.status === "ok" ? result.data : [];
    },
    staleTime: Infinity,
  });

  const localModels = supportedModels.data ?? [];
  const soniqoModels = localModels.filter((m) => m.model_type === "soniqo");

  const soniqoDownloaded = useQueries({
    queries: [...soniqoModels.map((m) => sttModelQueries.isDownloaded(m.key))],
  });

  const providers = Object.fromEntries(
    PROVIDERS.map((provider) => {
      const config = configuredProviders[providerRowId("stt", provider.id)] as
        | AIProviderStorage
        | undefined;
      const baseUrl = String(config?.base_url || provider.baseUrl || "").trim();
      const apiKey = String(config?.api_key || "").trim();

      const eligible =
        getProviderSelectionBlockers(provider.requirements, {
          isAuthenticated: true,
          isPaid: billing.isPaid,
          config: { base_url: baseUrl, api_key: apiKey },
        }).length === 0;

      if (!eligible) {
        return [provider.id, { configured: false, models: [] }];
      }

      if (provider.id === "hyprnote") {
        const models: ModelEntry[] = [
          { id: "cloud", isDownloaded: billing.isPaid, category: "latest" },
        ];

        if (isAppleSilicon) {
          soniqoModels.forEach((model, i) => {
            models.push({
              id: model.key,
              isDownloaded: soniqoDownloaded[i]?.data ?? false,
              displayName: model.display_name,
              sizeBytes: model.size_bytes,
              mode: isRealtimeLocalModel(String(model.key))
                ? "realtime"
                : "batch",
              category: "latest",
            });
          });
        }

        return [provider.id, { configured: true, models }];
      }

      if (provider.id === "custom") {
        return [provider.id, { configured: true, models: [] }];
      }

      return [
        provider.id,
        {
          configured: true,
          models: provider.models.map((model) => ({
            id: model,
            isDownloaded: true,
            mode: getProviderModelMode(provider.id, model),
          })),
        },
      ];
    }),
  ) as Record<
    ProviderId,
    {
      configured: boolean;
      models: ModelEntry[];
    }
  >;

  return { providers, isReady };
}

function ModelSelectItem({
  model,
  onDownload,
  onStartTrial,
}: {
  model: ModelEntry;
  onDownload: () => void;
  onStartTrial: () => void;
}) {
  const isCloud = model.id === "cloud";
  const { activeDownloads } = useNotifications();
  const downloadInfo = activeDownloads.find((d) => d.model === model.id);
  const isDownloading = !!downloadInfo;

  const label = displayModelLabel(model.id, model.displayName);
  const title = displayModelTitle(model.id, model.displayName);
  const sizeLabel = formatModelSize(model.sizeBytes);
  const showLocalActions = model.isDownloaded && isLocalModelId(model.id);
  const isDeprecated = model.isDeprecated === true;
  const content = (
    <div className="flex min-w-0 flex-1 items-center justify-between gap-3">
      <LocalModelLabel
        model={model.id}
        label={label}
        title={title}
        className="min-w-0 flex-1"
      />
      <div className="flex shrink-0 items-center gap-2 text-[11px]">
        <LocalModelBackendBadge model={model.id} />
        <ModelModeBadge mode={model.mode} />
        {!model.isDownloaded && sizeLabel && (
          <span className="text-muted-foreground font-mono">{sizeLabel}</span>
        )}
      </div>
    </div>
  );

  if (model.isDownloaded) {
    return (
      <div className="group/model-row relative overflow-hidden rounded-full">
        <SelectItem
          key={model.id}
          value={model.id}
          className={cn([
            "group-hover/model-row:bg-accent group-hover/model-row:text-accent-foreground",
            showLocalActions && "pr-20",
            isDeprecated && "text-muted-foreground focus:text-muted-foreground",
          ])}
        >
          {content}
        </SelectItem>
        {showLocalActions && (
          <LocalModelDropdownActions model={model.id as LocalModel} />
        )}
      </div>
    );
  }

  const handleAction = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (isDownloading) {
      return;
    }
    if (isCloud) {
      onStartTrial();
    } else {
      onDownload();
    }
  };

  return (
    <div
      className={cn([
        "relative flex items-center justify-between",
        "rounded-full py-1.5 text-sm outline-hidden",
        isCloud ? "pr-1.5 pl-2" : "px-2",
        "cursor-pointer select-none",
        "hover:bg-accent hover:text-accent-foreground",
        "group",
      ])}
    >
      <div className="text-muted-foreground min-w-0 flex-1">{content}</div>
      {isDownloading ? (
        <span
          className={cn([
            "rounded-full px-2 py-0.5 text-[11px] font-medium",
            "flex items-center gap-1",
            "from-muted to-accent text-muted-foreground bg-linear-to-t",
          ])}
        >
          <Loader2 className="size-3 animate-spin" />
          <span>{Math.round(downloadInfo.progress)}%</span>
        </span>
      ) : (
        <button
          className={cn([
            "rounded-full px-2 text-[11px] font-medium",
            "opacity-0 group-hover:opacity-100",
            "transition-all duration-150",
            isCloud
              ? "bg-primary text-primary-foreground hover:bg-primary/90 py-1 shadow-xs hover:shadow-md dark:!bg-white dark:!text-black dark:hover:!bg-white/90"
              : "from-muted to-accent text-foreground bg-linear-to-t py-0.5 shadow-xs hover:shadow-md",
          ])}
          onClick={handleAction}
        >
          {isCloud ? <Trans>Upgrade to use</Trans> : <Trans>Download</Trans>}
        </button>
      )}
    </div>
  );
}

function ModelSelectedValue({ model }: { model: ModelEntry }) {
  const isDeprecated = model.isDeprecated === true;

  return (
    <div className="flex max-w-full min-w-0 items-center gap-2">
      <LocalModelLabel
        model={model.id}
        label={displayModelLabel(model.id, model.displayName)}
        title={displayModelTitle(model.id, model.displayName)}
        className={cn(["min-w-0", isDeprecated && "opacity-60"])}
        labelClassName={cn([isDeprecated && "text-muted-foreground"])}
      />
      <ModelModeBadge mode={model.mode} />
    </div>
  );
}

function ModelModeBadge({ mode }: { mode?: ModelEntry["mode"] }) {
  if (!mode) {
    return null;
  }

  const isRealtime = mode === "realtime";

  return (
    <Tooltip delayDuration={100}>
      <TooltipTrigger asChild>
        <span
          className={cn([
            "shrink-0 cursor-help rounded-md px-1.5 py-0.5 text-[11px] font-medium",
            isRealtime
              ? "bg-sky-50 text-sky-700"
              : "bg-muted text-muted-foreground",
          ])}
        >
          {isRealtime ? <Trans>Live</Trans> : <Trans>After recording</Trans>}
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-64 text-xs">
        {isRealtime ? (
          <Trans>Can transcribe while the meeting is happening.</Trans>
        ) : (
          <Trans>
            Runs after the recording finishes, not during the meeting.
          </Trans>
        )}
      </TooltipContent>
    </Tooltip>
  );
}

function isLocalModelId(model: string): model is LocalModel {
  return isSupportedLocalSttModel(model);
}

function LocalModelDropdownActions({ model }: { model: LocalModel }) {
  const { t } = useLingui();
  const queryClient = useQueryClient();

  const stopSelect = (event: React.SyntheticEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
  };

  const handleOpen = () => {
    const resultPromise = String(model).startsWith("soniqo-")
      ? localSttCommands.soniqoModelDir(model)
      : localSttCommands.modelsDir();

    void resultPromise.then((result) => {
      if (result.status === "ok") {
        void openerCommands.openPath(result.data, null);
      }
    });
  };

  const handleDelete = () => {
    void localSttCommands.deleteModel(model).then((result) => {
      if (result.status === "ok") {
        void queryClient.invalidateQueries({
          queryKey: sttModelQueries.isDownloaded(model).queryKey,
        });
      }
    });
  };

  return (
    <div
      className={cn([
        "absolute top-0 right-0 bottom-0 z-10 flex items-center justify-end gap-1 rounded-r-full pl-6",
        "pointer-events-none opacity-0 transition-opacity duration-150",
        "group-hover/model-row:pointer-events-auto group-hover/model-row:opacity-100",
        "group-focus-within/model-row:pointer-events-auto group-focus-within/model-row:opacity-100",
      ])}
    >
      <button
        type="button"
        aria-label={t`Show in Finder`}
        className={cn([
          "flex size-6 items-center justify-center rounded-full",
          "text-muted-foreground hover:text-foreground",
        ])}
        onPointerDown={stopSelect}
        onClick={(event) => {
          stopSelect(event);
          handleOpen();
        }}
      >
        <FolderOpen className="size-3.5" />
      </button>
      <button
        type="button"
        aria-label={t`Delete model`}
        className={cn([
          "flex size-6 items-center justify-center rounded-full",
          "text-red-500 hover:text-red-600",
        ])}
        onPointerDown={stopSelect}
        onClick={(event) => {
          stopSelect(event);
          handleDelete();
        }}
      >
        <Trash2 className="size-3.5" />
      </button>
    </div>
  );
}
