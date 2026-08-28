import { Trans, useLingui } from "@lingui/react/macro";
import {
  Check,
  CircleNotch,
  FolderOpen,
  Trash,
  Warning,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import {
  commands as localSttCommands,
  type LocalModel,
} from "@anlg/plugin-local-stt";
import { commands as miscCommands } from "@anlg/plugin-misc";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import type { AIProviderStorage } from "@anlg/store";
import { Input } from "@anlg/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import { useSttSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import { LocalFileModel } from "./local-file-model";
import { LocalModelBackendBadge, LocalModelLabel } from "./model-icon";
import { recommendOnDeviceModel } from "./on-device-recommendation";
import {
  getDefaultSttSelection,
  getLanguageSupportIssue,
  resolveLiveLanguageSupportMode,
} from "./selection";
import {
  displayModelLabel,
  formatDownloadProgress,
  formatModelSize,
  isDeprecatedSttModel,
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
  canAppleSpeechTranscribe,
  isConfiguredSttModel,
  getSttModelTranscriptionMode,
  isDesktopLocalSttAvailable,
  isLiveTranscriptionSupported,
  isLocalFileSttModel,
  isOnDeviceSttModel,
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
      ? t`Choose a transcription model to start listening.`
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
    <div {...stylex.props(styles.container)}>
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
        lifecycle="condition-bound"
      />
      {!alertDescription && <TranscriptionLanguageWarningToast />}

      <h3 {...stylex.props(styles.heading)}>
        <Trans>Model being used</Trans>
      </h3>
      <div {...stylex.props(styles.selection)}>
        <div
          {...stylex.props(styles.providerControl)}
          data-stt-provider-selector
        >
          <Select value={visibleProvider} onValueChange={handleProviderChange}>
            <SelectTrigger sx={styles.providerTrigger}>
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
                    sx={[
                      styles.providerItem,
                      !configured && !locked && styles.unconfiguredProvider,
                    ]}
                  >
                    <div {...stylex.props(styles.providerOption)}>
                      <div {...stylex.props(styles.providerIdentity)}>
                        <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                        <span>{provider.displayName}</span>
                        {requiresPro ? (
                          <span {...stylex.props(styles.proBadge)}>
                            <Trans>Pro</Trans>
                          </span>
                        ) : null}
                      </div>
                      {locked ? (
                        <span {...stylex.props(styles.lockedDescription)}>
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

        <span {...stylex.props(styles.separator)}>/</span>

        {visibleProvider === "local_file" ? (
          <div {...stylex.props(styles.modelControl)}>
            <LocalFileModel healthStatus={health.status} />
          </div>
        ) : visibleProvider === "custom" ? (
          <div {...stylex.props(styles.modelControl)}>
            <Input
              value={displayedSttModel || ""}
              onChange={(event) => handleModelChange(event.target.value)}
              sx={styles.customModelInput}
              placeholder={t`Enter a model identifier`}
            />
          </div>
        ) : (
          <div {...stylex.props(styles.modelControl)}>
            <Select
              value={displayedSttModel || ""}
              onValueChange={handleModelChange}
              disabled={selectedModels.length === 0}
            >
              <SelectTrigger
                sx={[
                  styles.modelTrigger,
                  isConfigured && styles.configuredTrigger,
                ]}
              >
                <SelectValue
                  placeholder={t`Select a model`}
                  {...stylex.props(styles.modelValue)}
                >
                  {selectedModel ? (
                    <ModelSelectedValue model={selectedModel} />
                  ) : undefined}
                </SelectValue>
                {isConfigured && <HealthStatusIndicator />}
                {isConfigured && health.status === "success" && (
                  <Check {...stylex.props(styles.configuredIcon)} />
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
                        <div {...stylex.props(styles.category)}>
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
const MAX_DISMISSED_TRANSCRIPTION_LANGUAGE_WARNINGS = 128;
const DISMISSED_TRANSCRIPTION_LANGUAGE_WARNINGS_KEY =
  "anarlog:dismissed-transcription-language-warnings";

function rememberDismissedTranscriptionLanguageWarning(warningKey: string) {
  try {
    const warnings = readDismissedTranscriptionLanguageWarnings().filter(
      (key) => key !== warningKey,
    );
    warnings.push(warningKey);
    localStorage.setItem(
      DISMISSED_TRANSCRIPTION_LANGUAGE_WARNINGS_KEY,
      JSON.stringify(
        warnings.slice(-MAX_DISMISSED_TRANSCRIPTION_LANGUAGE_WARNINGS),
      ),
    );
  } catch {
    return;
  }
}

function isTranscriptionLanguageWarningDismissed(warningKey: string) {
  return readDismissedTranscriptionLanguageWarnings().includes(warningKey);
}

function readDismissedTranscriptionLanguageWarnings(): string[] {
  try {
    const stored = JSON.parse(
      localStorage.getItem(DISMISSED_TRANSCRIPTION_LANGUAGE_WARNINGS_KEY) ??
        "[]",
    );
    return Array.isArray(stored)
      ? stored.filter((key): key is string => typeof key === "string")
      : [];
  } catch {
    return [];
  }
}

function TranscriptionLanguageWarningToast() {
  const { i18n, t } = useLingui();
  const warning = useTranscriptionLanguageWarning();

  if (!warning || isTranscriptionLanguageWarningDismissed(warning.key)) {
    return null;
  }

  const model = displayModelLabel(warning.model);
  const unsupportedLanguages = warning.unsupportedLanguages.map((language) =>
    getBaseLanguageDisplayName(language, i18n.locale),
  );
  // Apple Speech is limited to languages added in System Settings, so a language it
  // supports needs a different fix than one it cannot transcribe at all.
  const needsSystemSettings =
    warning.model === "apple-speech"
      ? warning.unsupportedLanguages
          .filter((language) => canAppleSpeechTranscribe(language))
          .map((language) => getBaseLanguageDisplayName(language, i18n.locale))
      : [];

  const description =
    needsSystemSettings.length > 0
      ? t`Add ${formatLanguageList(needsSystemSettings)} in System Settings > General > Language & Region to transcribe with ${model}, or choose another model.`
      : unsupportedLanguages.length > 0
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
    let shouldRememberDismissal = true;
    sonnerToast.warning(description, {
      id: TRANSCRIPTION_LANGUAGE_WARNING_TOAST_ID,
      duration: Infinity,
      icon: <Warning {...stylex.props(styles.warningIcon)} />,
      action: {
        label: actionLabel,
        onClick: () => {
          shouldRememberDismissal = false;
          rememberDismissedTranscriptionLanguageWarning(warningKey);
          clearTranscriptionLanguageWarningToast();
        },
      },
      onDismiss: () => {
        if (shouldRememberDismissal) {
          rememberDismissedTranscriptionLanguageWarning(warningKey);
        }
      },
    });

    return () => {
      shouldRememberDismissal = false;
      clearTranscriptionLanguageWarningToast();
    };
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
  const isOnDeviceModel =
    isOnDeviceSttModel(current_stt_provider, selectedSttModel) ||
    isLocalFileSttModel(current_stt_provider, selectedSttModel);
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

type ModelCategory = "hardware" | "latest" | null;
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

  if (category === "hardware") {
    return <Trans>Best for this Mac</Trans>;
  }

  return null;
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
  const { local_stt_model_path } = useConfigValues([
    "local_stt_model_path",
  ] as const);

  const deviceInfo = useQuery({
    queryKey: ["device-info"],
    queryFn: async () => {
      const result = await miscCommands.getDeviceInfo(null);
      return result.status === "ok" ? result.data : null;
    },
    staleTime: Infinity,
  });

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
  // Listed only when the backend reports macOS 26 with Apple Speech available.
  const appleSpeechModels = localModels.filter(
    (m) => m.model_type === "appleSpeech",
  );

  const soniqoDownloaded = useQueries({
    queries: [...soniqoModels.map((m) => sttModelQueries.isDownloaded(m.key))],
  });

  const appleSpeechDownloaded = useQueries({
    queries: [
      ...appleSpeechModels.map((m) => sttModelQueries.isDownloaded(m.key)),
    ],
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

      if (provider.id === "anarlog") {
        return [
          provider.id,
          {
            configured: true,
            models: [
              {
                id: "cloud",
                isDownloaded: billing.isPaid,
                category: "latest" as const,
              },
            ],
          },
        ];
      }

      if (provider.id === "soniqo") {
        const models = buildOnDeviceModelEntries(
          soniqoModels,
          soniqoDownloaded,
          deviceInfo.data?.totalMemoryBytes,
        );
        return [provider.id, { configured: models.length > 0, models }];
      }

      if (provider.id === "apple_speech") {
        const models = buildOnDeviceModelEntries(
          appleSpeechModels,
          appleSpeechDownloaded,
          deviceInfo.data?.totalMemoryBytes,
        );
        return [provider.id, { configured: models.length > 0, models }];
      }

      if (provider.id === "local_file") {
        const available = isDesktopLocalSttAvailable(
          deviceInfo.data?.platform ?? "",
          deviceInfo.data?.arch ?? "",
        );
        return [
          provider.id,
          {
            configured: available,
            models: [
              {
                id: "local-file",
                isDownloaded: !!local_stt_model_path?.trim(),
                mode: "batch" as const,
              },
            ],
          },
        ];
      }

      if (provider.id === "custom") {
        return [provider.id, { configured: true, models: [] }];
      }

      return [
        provider.id,
        {
          configured: true,
          models: provider.models.map((model) => {
            const mode = getSttModelTranscriptionMode(provider.id, model);
            return {
              id: model,
              isDownloaded: true,
              mode: mode === "live" ? "realtime" : mode,
              isDeprecated: isDeprecatedSttModel(provider.id, model),
            };
          }),
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

  return {
    providers,
    isReady: isReady && supportedModels.isFetched && deviceInfo.isFetched,
  };
}

function buildOnDeviceModelEntries(
  models: Array<{
    key: LocalModel;
    display_name: string;
    size_bytes: number | null;
    supports_realtime: boolean;
    recommended_memory_bytes: number;
  }>,
  downloads: Array<{ data?: boolean }>,
  totalMemoryBytes?: number,
): ModelEntry[] {
  const recommendedModel = recommendOnDeviceModel(
    models.map((model) => ({
      id: model.key,
      recommendedMemoryBytes: model.recommended_memory_bytes,
    })),
    totalMemoryBytes,
  );

  return models
    .map((model, index) => ({
      id: model.key,
      isDownloaded: downloads[index]?.data ?? false,
      displayName: model.display_name,
      sizeBytes: model.size_bytes,
      mode: model.supports_realtime
        ? ("realtime" as const)
        : ("batch" as const),
      category: model.key === recommendedModel ? ("hardware" as const) : null,
    }))
    .sort(
      (a, b) =>
        Number(b.id === recommendedModel) - Number(a.id === recommendedModel),
    );
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
  const { queuedDownloads } = useSttSettings();
  const downloadInfo = activeDownloads.find((d) => d.model === model.id);
  const isDownloading =
    !!downloadInfo || queuedDownloads.includes(model.id as LocalModel);
  const [isInteractive, setIsInteractive] = useState(false);

  const label = displayModelLabel(model.id, model.displayName);
  const sizeLabel = formatModelSize(model.sizeBytes);
  const showLocalActions = model.isDownloaded && isLocalModelId(model.id);
  const isDeprecated = model.isDeprecated === true;
  const content = (
    <div {...stylex.props(styles.modelItemContent)}>
      <LocalModelLabel
        model={model.id}
        label={label}
        title={label}
        sx={styles.modelLabel}
      />
      <div {...stylex.props(styles.modelMetadata)}>
        <LocalModelBackendBadge model={model.id} />
        {isDeprecated && <DeprecatedBadge />}
        {model.mode !== "realtime" && <ModelModeBadge mode={model.mode} />}
        {!model.isDownloaded && sizeLabel && (
          <span {...stylex.props(styles.modelSize)}>{sizeLabel}</span>
        )}
      </div>
    </div>
  );

  if (model.isDownloaded) {
    return (
      <div
        {...stylex.props(styles.downloadedRow)}
        onPointerEnter={() => setIsInteractive(true)}
        onPointerLeave={() => setIsInteractive(false)}
        onFocusCapture={() => setIsInteractive(true)}
        onBlurCapture={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget)) {
            setIsInteractive(false);
          }
        }}
      >
        <SelectItem
          key={model.id}
          value={model.id}
          sx={[
            isInteractive && styles.interactiveModelItem,
            showLocalActions && styles.modelItemWithActions,
            isDeprecated && styles.deprecatedModelItem,
          ]}
        >
          {content}
        </SelectItem>
        {showLocalActions && (
          <LocalModelDropdownActions
            model={model.id as LocalModel}
            visible={isInteractive}
          />
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
      {...stylex.props(
        styles.unavailableModel,
        isCloud ? styles.cloudModelPadding : styles.localModelPadding,
      )}
      onPointerEnter={() => setIsInteractive(true)}
      onPointerLeave={() => setIsInteractive(false)}
      onFocusCapture={() => setIsInteractive(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          setIsInteractive(false);
        }
      }}
    >
      <div {...stylex.props(styles.unavailableContent)}>{content}</div>
      {isDownloading ? (
        <span {...stylex.props(styles.downloadProgress)}>
          <CircleNotch {...stylex.props(styles.progressSpinner)} />
          {downloadInfo ? (
            formatDownloadProgress(downloadInfo.progress)
          ) : (
            <Trans>Starting</Trans>
          )}
        </span>
      ) : (
        <button
          {...stylex.props(
            styles.modelActionButton,
            isInteractive && styles.visibleModelAction,
            isCloud ? styles.cloudModelAction : styles.downloadModelAction,
          )}
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
  const label = displayModelLabel(model.id, model.displayName);

  return (
    <div {...stylex.props(styles.selectedModel)}>
      <LocalModelLabel
        model={model.id}
        label={label}
        title={label}
        sx={[styles.selectedModelLabel, isDeprecated && styles.faded]}
        labelSx={isDeprecated && styles.deprecatedLabel}
      />
      {isDeprecated && <DeprecatedBadge />}
      <ModelModeBadge mode={model.mode} />
    </div>
  );
}

function DeprecatedBadge() {
  return (
    <span {...stylex.props(styles.deprecatedBadge)}>
      <Trans>Deprecated</Trans>
    </span>
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
          {...stylex.props(
            styles.modeBadge,
            isRealtime ? styles.liveBadge : styles.batchBadge,
          )}
        >
          {isRealtime ? <Trans>Live</Trans> : <Trans>After recording</Trans>}
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" sx={styles.modeTooltip}>
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

function LocalModelDropdownActions({
  model,
  visible,
}: {
  model: LocalModel;
  visible: boolean;
}) {
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

  const deleteModel = useMutation({
    mutationFn: () => localSttCommands.deleteModel(model),
    onSuccess: (result) => {
      if (result.status === "ok") {
        void queryClient.invalidateQueries({
          queryKey: sttModelQueries.isDownloaded(model).queryKey,
        });
      }
    },
  });

  const handleDelete = () => {
    if (deleteModel.isPending) {
      return;
    }
    deleteModel.mutate();
  };

  return (
    <div
      data-model-actions-pending={deleteModel.isPending || undefined}
      {...stylex.props(
        styles.modelActions,
        (visible || deleteModel.isPending) && styles.visibleModelActions,
      )}
    >
      <button
        type="button"
        aria-label={t`Show in Finder`}
        {...stylex.props(styles.modelActionIconButton)}
        onPointerDown={stopSelect}
        onClick={(event) => {
          stopSelect(event);
          handleOpen();
        }}
      >
        <FolderOpen {...stylex.props(styles.smallIcon)} />
      </button>
      <button
        type="button"
        aria-label={t`Delete model`}
        disabled={deleteModel.isPending}
        {...stylex.props(styles.deleteModelButton)}
        onPointerDown={stopSelect}
        onClick={(event) => {
          stopSelect(event);
          handleDelete();
        }}
      >
        {deleteModel.isPending ? (
          <CircleNotch {...stylex.props(styles.deletingSpinner)} />
        ) : (
          <Trash {...stylex.props(styles.smallIcon)} />
        )}
      </button>
    </div>
  );
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  batchBadge: {
    backgroundColor: colors.muted,
    color: colors.mutedForeground,
  },
  category: {
    color: colors.mutedForeground,
    fontSize: "11px",
    fontWeight: 500,
    letterSpacing: "0.025em",
    paddingBottom: "0.25rem",
    paddingInline: "0.5rem",
    paddingTop: "0.5rem",
    textTransform: "uppercase",
  },
  cloudModelAction: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
      ":is(.dark *)": "white",
      ":is(.dark *):hover": "rgb(255 255 255 / 0.9)",
    },
    boxShadow: {
      default: "0 1px 2px rgb(0 0 0 / 0.05)",
      ":hover":
        "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    },
    color: {
      default: colors.primaryForeground,
      ":is(.dark *)": "black",
    },
    paddingBlock: "0.25rem",
  },
  cloudModelPadding: {
    paddingLeft: "0.5rem",
    paddingRight: "0.375rem",
  },
  configuredIcon: {
    backgroundColor: colors.card,
    color: "rgb(22 163 74)",
    flexShrink: 0,
    height: "1rem",
    position: "absolute",
    right: "0.75rem",
    width: "1rem",
    zIndex: 1,
  },
  configuredTrigger: {
    isolation: "isolate",
    position: "relative",
  },
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  customModelInput: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  deleteModelButton: {
    alignItems: "center",
    borderRadius: radii.full,
    color: {
      default: "rgb(239 68 68)",
      ":hover": "rgb(220 38 38)",
    },
    display: "flex",
    height: "1.5rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.7,
    },
    width: "1.5rem",
  },
  deletingSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.875rem",
    width: "0.875rem",
  },
  deprecatedBadge: {
    backgroundColor: "rgb(255 251 235)",
    borderRadius: radii.md,
    color: "rgb(146 64 14)",
    flexShrink: 0,
    fontSize: "11px",
    fontWeight: 500,
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  deprecatedLabel: {
    color: colors.mutedForeground,
  },
  deprecatedModelItem: {
    color: {
      default: colors.mutedForeground,
      ":focus": colors.mutedForeground,
    },
  },
  downloadedRow: {
    borderRadius: radii.full,
    overflow: "hidden",
    position: "relative",
  },
  downloadModelAction: {
    backgroundImage: `linear-gradient(to top, ${colors.muted}, ${colors.accent})`,
    boxShadow: {
      default: "0 1px 2px rgb(0 0 0 / 0.05)",
      ":hover":
        "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    },
    color: colors.foreground,
    paddingBlock: "0.125rem",
  },
  downloadProgress: {
    alignItems: "center",
    backgroundImage: `linear-gradient(to top, ${colors.muted}, ${colors.accent})`,
    borderRadius: radii.full,
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "11px",
    fontWeight: 500,
    gap: "0.25rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
  },
  faded: {
    opacity: 0.6,
  },
  heading: {
    fontFamily: fonts.sans,
    fontSize: "1rem",
    fontWeight: 600,
  },
  interactiveModelItem: {
    backgroundColor: colors.accent,
    color: colors.accentForeground,
  },
  liveBadge: {
    backgroundColor: "rgb(240 249 255)",
    color: "rgb(3 105 161)",
  },
  localModelPadding: {
    paddingInline: "0.5rem",
  },
  lockedDescription: {
    color: colors.mutedForeground,
    fontSize: "11px",
  },
  modeBadge: {
    borderRadius: radii.md,
    cursor: "help",
    flexShrink: 0,
    fontSize: "11px",
    fontWeight: 500,
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  modeTooltip: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
    maxWidth: "16rem",
  },
  modelActionButton: {
    borderRadius: radii.full,
    fontSize: "11px",
    fontWeight: 500,
    opacity: 0,
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  modelActionIconButton: {
    alignItems: "center",
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  modelActions: {
    alignItems: "center",
    borderBottomRightRadius: radii.full,
    borderTopRightRadius: radii.full,
    bottom: 0,
    display: "flex",
    gap: "0.25rem",
    justifyContent: "flex-end",
    opacity: 0,
    paddingLeft: "1.5rem",
    pointerEvents: "none",
    position: "absolute",
    right: 0,
    top: 0,
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  modelControl: {
    flex: "3",
    minWidth: 0,
  },
  modelItemContent: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.75rem",
    justifyContent: "space-between",
    minWidth: 0,
  },
  modelItemWithActions: {
    paddingRight: "5rem",
  },
  modelLabel: {
    flex: "1",
    minWidth: 0,
  },
  modelMetadata: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    fontSize: "11px",
    gap: "0.5rem",
  },
  modelSize: {
    color: colors.mutedForeground,
    fontFamily: fonts.mono,
  },
  modelTrigger: {
    backgroundColor: colors.card,
    boxShadow: {
      default: "none",
      ":focus": "none",
    },
    textAlign: "left",
  },
  modelValue: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "flex-start",
    minWidth: 0,
    overflow: "visible",
    width: "100%",
    WebkitLineClamp: "unset",
  },
  proBadge: {
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    fontSize: "10px",
    letterSpacing: "0.025em",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
    textTransform: "uppercase",
  },
  progressSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.75rem",
    width: "0.75rem",
  },
  providerControl: {
    flex: "2",
    minWidth: 0,
  },
  providerIdentity: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  providerItem: {
    color: {
      default: null,
      ":is([data-disabled])": colors.mutedForeground,
    },
    opacity: {
      default: null,
      ":is([data-disabled])": 1,
    },
  },
  providerOption: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  providerTrigger: {
    backgroundColor: colors.card,
    boxShadow: {
      default: "none",
      ":focus": "none",
    },
  },
  selectedModel: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    maxWidth: "100%",
    minWidth: 0,
  },
  selectedModelLabel: {
    minWidth: 0,
  },
  selection: {
    alignItems: "center",
    display: "flex",
    flexDirection: "row",
    gap: "1rem",
  },
  separator: {
    color: colors.mutedForeground,
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  unavailableContent: {
    color: colors.mutedForeground,
    flex: "1",
    minWidth: 0,
  },
  unavailableModel: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: null,
      ":hover": colors.accentForeground,
    },
    cursor: "pointer",
    display: "flex",
    fontSize: "0.875rem",
    justifyContent: "space-between",
    lineHeight: "1.25rem",
    outlineWidth: "2px",
    outlineStyle: "solid",
    outlineColor: "transparent",
    outlineOffset: "2px",
    paddingBlock: "0.375rem",
    position: "relative",
    userSelect: "none",
  },
  unconfiguredProvider: {
    color: colors.mutedForeground,
  },
  visibleModelAction: {
    opacity: 1,
  },
  visibleModelActions: {
    opacity: 1,
    pointerEvents: "auto",
  },
  warningIcon: {
    color: "rgb(245 158 11)",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
});
