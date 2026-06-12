import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { arch } from "@tauri-apps/plugin-os";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  Loader2,
  Trash2,
} from "lucide-react";
import { useRef } from "react";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as listenerCommands } from "@hypr/plugin-transcription";
import type { AIProviderStorage } from "@hypr/store";
import { Input } from "@hypr/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { cn } from "@hypr/utils";

import { useSttSettings } from "./context";
import { HealthStatusIndicator, useConnectionHealth } from "./health";
import { LocalModelBackendBadge, LocalModelLabel } from "./model-icon";
import { getPreferredProviderModel } from "./selection";
import {
  displayModelId,
  formatModelSize,
  type ProviderId,
  PROVIDERS,
  sttModelQueries,
} from "./shared";

import { useBillingAccess } from "~/auth/billing";
import { useNotifications } from "~/contexts/notifications";
import { providerRowId, ProviderIconSlot } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { useConfigValues } from "~/shared/config";
import { SettingsAlert } from "~/shared/ui/settings-alert";
import * as settings from "~/store/tinybase/store/settings";
import {
  isConfiguredSttModel,
  isHyprnoteLocalSttModel,
  isLiveTranscriptionSupported,
  isRealtimeLocalModel,
  isSupportedLocalSttModel,
} from "~/stt/capabilities";

export function SelectProviderAndModel() {
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const billing = useBillingAccess();
  const configuredProviders = useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const health = useConnectionHealth();

  const selectedSttModel = isConfiguredSttModel(
    current_stt_provider,
    current_stt_model,
  )
    ? current_stt_model
    : undefined;
  const isConfigured = !!(current_stt_provider && selectedSttModel);
  const hasError = isConfigured && health.status === "error";
  const selectedProvider = current_stt_provider as ProviderId | undefined;
  const selectedModels = selectedProvider
    ? (configuredProviders[selectedProvider]?.models ?? [])
    : [];
  const selectedModel = selectedModels.find(
    (model) => model.id === selectedSttModel,
  );

  const handleSelectProvider = settings.UI.useSetValueCallback(
    "current_stt_provider",
    (provider: string) => provider,
    [],
    settings.STORE_ID,
  );

  const handleSelectModel = settings.UI.useSetValueCallback(
    "current_stt_model",
    (model: string) => model,
    [],
    settings.STORE_ID,
  );
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
    const nextModel = getPreferredProviderModel(
      lastSelectedModelsRef.current[provider],
      nextModels,
      { allowSavedModelWithoutChoices: providerId === "custom" },
    );

    rememberModel(provider, nextModel);
    handleSelectProvider(provider);
    handleSelectModel(nextModel);
  };

  const handleModelChange = (model: string) => {
    if (!current_stt_provider) {
      return;
    }

    rememberModel(current_stt_provider, model);
    handleSelectModel(model);
  };
  return (
    <div className="flex flex-col gap-4">
      {!isConfigured && (
        <SettingsAlert>
          <strong className="font-medium">Transcription model</strong> is needed
          to make Anarlog listen to your conversations.
        </SettingsAlert>
      )}

      {hasError && health.message && (
        <SettingsAlert>{health.message}</SettingsAlert>
      )}

      <h3 className="text-md font-sans font-semibold">Model being used</h3>
      <div className="flex flex-row items-center gap-4">
        <div className="min-w-0 flex-2" data-stt-provider-selector>
          <Select
            value={current_stt_provider || ""}
            onValueChange={handleProviderChange}
          >
            <SelectTrigger className="bg-card shadow-none focus:ring-0">
              <SelectValue placeholder="Select a provider" />
            </SelectTrigger>
            <SelectContent>
              {PROVIDERS.filter(({ disabled }) => !disabled).map((provider) => {
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
                            Pro
                          </span>
                        ) : null}
                      </div>
                      {locked ? (
                        <span className="text-muted-foreground text-[11px]">
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

        <span className="text-muted-foreground">/</span>

        {current_stt_provider === "custom" ? (
          <div className="min-w-0 flex-3">
            <Input
              value={selectedSttModel || ""}
              onChange={(event) => handleModelChange(event.target.value)}
              className="text-xs"
              placeholder="Enter a model identifier"
            />
          </div>
        ) : (
          <div className="min-w-0 flex-3">
            <Select
              value={selectedSttModel || ""}
              onValueChange={handleModelChange}
              disabled={selectedModels.length === 0}
            >
              <SelectTrigger
                className={cn([
                  "bg-card text-left shadow-none focus:ring-0",
                  "[&>span]:flex [&>span]:w-full [&>span]:items-center [&>span]:justify-between [&>span]:gap-2",
                  isConfigured && "[&>svg:last-child]:hidden",
                ])}
              >
                <SelectValue placeholder="Select a model">
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

export function TranscriptionLanguageWarningBanner() {
  const hasLanguageWarning = useHasLanguageWarning();

  if (!hasLanguageWarning) {
    return null;
  }

  return (
    <div className="-mx-6 -mt-6 mb-6 border-b border-amber-200 bg-amber-50 px-6 py-3">
      <span className="flex items-center justify-center gap-2 text-center text-sm text-amber-600">
        <AlertTriangle className="size-4 shrink-0" />
        Selected model may not support all your spoken languages.
      </span>
    </div>
  );
}

function useHasLanguageWarning() {
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

  const languageSupport = useQuery({
    queryKey: [
      "stt-language-support",
      current_stt_provider,
      selectedSttModel,
      useLiveOnDeviceModel,
      liveSupport.data,
      spoken_languages,
    ],
    queryFn: async () => {
      const useLiveMode = isOnDeviceModel
        ? useLiveOnDeviceModel && liveSupport.data
        : liveSupport.data;
      const result = useLiveMode
        ? await listenerCommands.isSupportedLanguagesLive(
            current_stt_provider!,
            selectedSttModel ?? null,
            spoken_languages ?? [],
          )
        : await listenerCommands.isSupportedLanguagesBatch(
            current_stt_provider!,
            selectedSttModel ?? null,
            spoken_languages ?? [],
          );
      return result.status === "ok" ? result.data : true;
    },
    enabled:
      isConfigured &&
      liveSupport.data !== undefined &&
      !!spoken_languages?.length,
  });

  return isConfigured && languageSupport.data === false && !hasError;
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
    if (model === "stt-v5" || model === "stt-async-v5") {
      return "batch";
    }

    if (model === "stt-v4" || model === "stt-rt-v4") {
      return "realtime";
    }
  }

  return undefined;
}

function useConfiguredMapping(): Record<
  ProviderId,
  {
    configured: boolean;
    models: ModelEntry[];
  }
> {
  const billing = useBillingAccess();
  const configuredProviders = settings.UI.useResultTable(
    settings.QUERIES.sttProviders,
    settings.STORE_ID,
  );

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

  return Object.fromEntries(
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

  const label = model.displayName ?? displayModelId(model.id);
  const sizeLabel = formatModelSize(model.sizeBytes);
  const showLocalActions = model.isDownloaded && isLocalModelId(model.id);
  const isDeprecated = model.isDeprecated === true;
  const content = (
    <div className="flex min-w-0 flex-1 items-center justify-between gap-3">
      <LocalModelLabel
        model={model.id}
        label={label}
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
      <div className="group/model-row relative">
        <SelectItem
          key={model.id}
          value={model.id}
          className={cn([
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
        "rounded-full px-2 py-1.5 text-sm outline-hidden",
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
              ? "bg-primary text-primary-foreground hover:bg-primary/90 py-1 shadow-xs hover:shadow-md"
              : "from-muted to-accent text-foreground bg-linear-to-t py-0.5 shadow-xs hover:shadow-md",
          ])}
          onClick={handleAction}
        >
          {isCloud ? "Upgrade to use" : "Download"}
        </button>
      )}
    </div>
  );
}

function ModelSelectedValue({ model }: { model: ModelEntry }) {
  const isDeprecated = model.isDeprecated === true;

  return (
    <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
      <LocalModelLabel
        model={model.id}
        label={model.displayName ?? displayModelId(model.id)}
        className={cn(["min-w-0 flex-1", isDeprecated && "opacity-60"])}
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

  return (
    <span
      className={cn([
        "shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        mode === "realtime"
          ? "bg-sky-50 text-sky-700"
          : "bg-muted text-muted-foreground",
      ])}
    >
      {mode === "realtime" ? "Realtime" : "Batch"}
    </span>
  );
}

function isLocalModelId(model: string): model is LocalModel {
  return isSupportedLocalSttModel(model);
}

function LocalModelDropdownActions({ model }: { model: LocalModel }) {
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
        "absolute top-0 right-1 bottom-0 z-10 flex items-center justify-end gap-1 pl-6",
        "via-accent/95 to-accent bg-linear-to-r from-transparent",
        "pointer-events-none opacity-0 transition-opacity duration-150",
        "group-hover/model-row:pointer-events-auto group-hover/model-row:opacity-100",
        "group-focus-within/model-row:pointer-events-auto group-focus-within/model-row:opacity-100",
      ])}
    >
      <button
        type="button"
        aria-label="Show in Finder"
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
        aria-label="Delete model"
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
