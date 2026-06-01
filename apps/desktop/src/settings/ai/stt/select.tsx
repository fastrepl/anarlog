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
import * as settings from "~/store/tinybase/store/settings";
import {
  isLiveTranscriptionSupported,
  isRealtimeLocalModel,
} from "~/stt/capabilities";

export function SelectProviderAndModel() {
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const billing = useBillingAccess();
  const configuredProviders = useConfiguredMapping(current_stt_model);
  const { startDownload, startTrial } = useSttSettings();
  const health = useConnectionHealth();

  const isConfigured = !!(current_stt_provider && current_stt_model);
  const hasError = isConfigured && health.status === "error";
  const selectedProvider = current_stt_provider as ProviderId | undefined;
  const selectedModels = selectedProvider
    ? (configuredProviders[selectedProvider]?.models ?? [])
    : [];
  const selectedModel = selectedModels.find(
    (model) => model.id === current_stt_model,
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
    current_stt_provider && current_stt_model
      ? { [current_stt_provider]: current_stt_model }
      : {},
  );
  const rememberModel = (provider?: string, model?: string) => {
    if (!provider || model === undefined) {
      return;
    }

    lastSelectedModelsRef.current[provider] = model;
  };

  const handleProviderChange = (provider: string) => {
    rememberModel(current_stt_provider, current_stt_model);

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
        <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3">
          <span className="text-sm text-red-600">
            <strong className="font-medium">Transcription model</strong> is
            needed to make Anarlog listen to your conversations.
          </span>
        </div>
      )}

      {hasError && health.message && (
        <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3">
          <span className="text-sm text-red-600">{health.message}</span>
        </div>
      )}

      <h3 className="text-md font-sans font-semibold">Model being used</h3>
      <div className="flex flex-row items-center gap-4">
        <div className="min-w-0 flex-2" data-stt-provider-selector>
          <Select
            value={current_stt_provider || ""}
            onValueChange={handleProviderChange}
          >
            <SelectTrigger className="bg-white shadow-none focus:ring-0">
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
                    disabled={provider.disabled || !configured || locked}
                  >
                    <div className="flex flex-col gap-0.5">
                      <div className="flex items-center gap-2">
                        <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
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

        {current_stt_provider === "custom" ? (
          <div className="min-w-0 flex-3">
            <Input
              value={current_stt_model || ""}
              onChange={(event) => handleModelChange(event.target.value)}
              className="text-xs"
              placeholder="Enter a model identifier"
            />
          </div>
        ) : (
          <div className="min-w-0 flex-3">
            <Select
              value={current_stt_model || ""}
              onValueChange={handleModelChange}
              disabled={selectedModels.length === 0}
            >
              <SelectTrigger
                className={cn([
                  "bg-white text-left shadow-none focus:ring-0",
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
                        <div className="px-2 pt-2 pb-1 text-[11px] font-medium tracking-wide text-neutral-400 uppercase">
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

  const isConfigured = !!(current_stt_provider && current_stt_model);
  const isOnDeviceModel =
    current_stt_provider === "hyprnote" &&
    !!current_stt_model &&
    current_stt_model !== "cloud";
  const useLiveOnDeviceModel =
    isOnDeviceModel && isRealtimeLocalModel(current_stt_model);
  const hasError = isConfigured && health.status === "error";
  const liveSupport = useQuery({
    queryKey: ["stt-live-support", current_stt_provider, current_stt_model],
    queryFn: () =>
      isLiveTranscriptionSupported(current_stt_provider, current_stt_model),
    enabled: isConfigured,
  });

  const languageSupport = useQuery({
    queryKey: [
      "stt-language-support",
      current_stt_provider,
      current_stt_model,
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
            current_stt_model ?? null,
            spoken_languages ?? [],
          )
        : await listenerCommands.isSupportedLanguagesBatch(
            current_stt_provider!,
            current_stt_model ?? null,
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

type ModelCategory = "latest" | "deprecated" | null;
type ModelEntry = {
  id: string;
  isDownloaded: boolean;
  displayName?: string;
  category?: ModelCategory;
  sizeBytes?: number | null;
  mode?: "realtime" | "batch";
  status?: "deprecated";
};

function getModelCategoryLabel(category?: ModelCategory) {
  if (category === "latest") {
    return "Recommended";
  }

  if (category === "deprecated") {
    return "Cactus (Deprecated)";
  }

  return null;
}

function useConfiguredMapping(currentModel?: string): Record<
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
  const cactusModels = localModels.filter((m) => m.model_type === "cactus");
  const visibleCactusModels = cactusModels.filter(
    (m) => m.key === currentModel,
  );
  const soniqoModels = localModels.filter((m) => m.model_type === "soniqo");

  const cactusDownloaded = useQueries({
    queries: [
      ...visibleCactusModels.map((m) => sttModelQueries.isDownloaded(m.key)),
    ],
  });
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

          const sorted = [...visibleCactusModels].sort((a) =>
            String(a.key).includes("parakeet") ? -1 : 1,
          );
          sorted.forEach((model) => {
            const i = visibleCactusModels.indexOf(model);
            models.push({
              id: model.key,
              isDownloaded: cactusDownloaded[i]?.data ?? false,
              displayName: model.display_name,
              sizeBytes: model.size_bytes,
              mode: "batch",
              category: "deprecated",
              status: "deprecated",
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
  const isDeprecated = model.status === "deprecated";
  const content = (
    <div
      className={cn([
        "flex min-w-0 flex-1 items-center justify-between gap-3",
        isDeprecated && "opacity-60",
      ])}
    >
      <LocalModelLabel
        model={model.id}
        label={label}
        className="min-w-0 flex-1"
      />
      <div className="flex shrink-0 items-center gap-2 text-[11px]">
        <LocalModelBackendBadge model={model.id} />
        {model.status === "deprecated" && (
          <span className="rounded-md bg-amber-50 px-1.5 py-0.5 font-medium text-amber-700">
            Deprecated
          </span>
        )}
        {model.mode && (
          <span
            className={cn([
              "rounded-md px-1.5 py-0.5 font-medium",
              model.mode === "realtime"
                ? "bg-sky-50 text-sky-700"
                : "bg-neutral-100 text-neutral-600",
            ])}
          >
            {model.mode === "realtime" ? "Realtime" : "Batch"}
          </span>
        )}
        {!model.isDownloaded && sizeLabel && (
          <span className="font-mono text-neutral-500">{sizeLabel}</span>
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
            isDeprecated && "text-neutral-400 focus:text-neutral-500",
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
      <div className="min-w-0 flex-1 text-neutral-400">{content}</div>
      {isDownloading ? (
        <span
          className={cn([
            "rounded-full px-2 py-0.5 text-[11px] font-medium",
            "flex items-center gap-1",
            "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-500",
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
              ? "bg-linear-to-t from-stone-600 to-stone-500 py-1 text-white shadow-xs hover:shadow-md"
              : "bg-linear-to-t from-neutral-200 to-neutral-100 py-0.5 text-neutral-900 shadow-xs hover:shadow-md",
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
  return (
    <LocalModelLabel
      model={model.id}
      label={model.displayName ?? displayModelId(model.id)}
      className={cn([
        "min-w-0 flex-1",
        model.status === "deprecated" && "opacity-60",
      ])}
      labelClassName={cn([model.status === "deprecated" && "text-neutral-400"])}
    />
  );
}

function isLocalModelId(model: string): model is LocalModel {
  return (
    model.startsWith("soniqo-") ||
    model.startsWith("cactus-") ||
    model.startsWith("am-") ||
    model.startsWith("Quantized")
  );
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
      : String(model).startsWith("cactus-")
        ? localSttCommands.cactusModelsDir()
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
          "text-neutral-500 hover:text-neutral-900",
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
