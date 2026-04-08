import { useQueries, useQuery } from "@tanstack/react-query";
import { arch } from "@tauri-apps/plugin-os";
import { Check, Loader2 } from "lucide-react";
import { useRef } from "react";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";
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
import { getPreferredProviderModel } from "./selection";
import {
  displayModelId,
  type ProviderId,
  PROVIDERS,
  sttModelQueries,
} from "./shared";

import { useBillingAccess } from "~/auth/billing";
import { useNotifications } from "~/contexts/notifications";
import { providerRowId } from "~/settings/ai/shared";
import {
  getProviderSelectionBlockers,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";
import { isLiveTranscriptionSupported } from "~/stt/capabilities";

export function SelectProviderAndModel() {
  const { current_stt_provider, current_stt_model, spoken_languages } =
    useConfigValues([
      "current_stt_provider",
      "current_stt_model",
      "spoken_languages",
    ] as const);
  const billing = useBillingAccess();
  const configuredProviders = useConfiguredMapping();
  const { startDownload, startTrial } = useSttSettings();
  const health = useConnectionHealth();

  const isConfigured = !!(current_stt_provider && current_stt_model);
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
      liveSupport.data,
      spoken_languages,
    ],
    queryFn: async () => {
      const result = liveSupport.data
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

  const hasLanguageWarning =
    isConfigured && languageSupport.data === false && !hasError;
  const selectedProvider = current_stt_provider as ProviderId | undefined;
  const selectedModels = selectedProvider
    ? (configuredProviders[selectedProvider]?.models ?? [])
    : [];

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
    <div className="flex flex-col gap-3">
      <h3 className="text-md font-serif font-semibold">Model being used</h3>
      <div
        className={cn([
          "flex flex-col gap-4",
          "rounded-xl border border-neutral-200 p-4",
          !isConfigured || hasError
            ? "bg-red-50"
            : hasLanguageWarning
              ? "bg-amber-50"
              : "bg-neutral-50",
        ])}
      >
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
                {PROVIDERS.filter(({ disabled }) => !disabled).map(
                  (provider) => {
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
                  },
                )}
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
                  <SelectValue placeholder="Select a model" />
                  {isConfigured && <HealthStatusIndicator />}
                  {isConfigured && health.status === "success" && (
                    <Check className="-mr-1 h-4 w-4 shrink-0 text-green-600" />
                  )}
                </SelectTrigger>
                <SelectContent>
                  {selectedModels.map((model, i) => {
                    const prevCategory =
                      i > 0 ? selectedModels[i - 1].category : null;
                    const showHeader =
                      model.category && model.category !== prevCategory;
                    return (
                      <span key={model.id}>
                        {showHeader && (
                          <div className="px-2 pt-2 pb-1 text-[11px] font-medium tracking-wide text-neutral-400 uppercase">
                            {model.category === "experimental"
                              ? "Others"
                              : "Recommended"}
                          </div>
                        )}
                        <ModelSelectItem
                          model={model}
                          onDownload={() =>
                            startDownload(model.id as LocalModel)
                          }
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

        {!isConfigured && (
          <div className="flex items-center gap-2 border-t border-red-200 pt-2">
            <span className="text-sm text-red-600">
              <strong className="font-medium">Transcription model</strong> is
              needed to make Char listen to your conversations.
            </span>
          </div>
        )}

        {hasError && health.message && (
          <div className="flex items-center gap-2 border-t border-red-200 pt-2">
            <span className="text-sm text-red-600">{health.message}</span>
          </div>
        )}
        {hasLanguageWarning && (
          <div className="flex items-center gap-2 border-t border-amber-200 pt-2">
            <span className="text-sm text-amber-600">
              Selected model may not support all your spoken languages.
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

type ModelCategory = "latest" | "experimental" | null;
type ModelEntry = {
  id: string;
  isDownloaded: boolean;
  displayName?: string;
  category?: ModelCategory;
};

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

  const cactusModels =
    supportedModels.data?.filter((m) => m.model_type === "cactus") ?? [];

  const cactusDownloaded = useQueries({
    queries: [...cactusModels.map((m) => sttModelQueries.isDownloaded(m.key))],
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
          const sorted = [...cactusModels].sort((a) =>
            String(a.key).includes("parakeet") ? -1 : 1,
          );
          sorted.forEach((model) => {
            const i = cactusModels.indexOf(model);
            const isRecommended = String(model.key).includes("parakeet");
            models.push({
              id: model.key,
              isDownloaded: cactusDownloaded[i]?.data ?? false,
              displayName: model.display_name,
              category: isRecommended ? "latest" : "experimental",
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
  const billing = useBillingAccess();

  const label = model.displayName ?? displayModelId(model.id);

  if (model.isDownloaded) {
    return (
      <SelectItem key={model.id} value={model.id}>
        {label}
      </SelectItem>
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

  const cloudButtonLabel = billing.canStartTrial.data
    ? "Free Trial"
    : "Upgrade";

  return (
    <div
      className={cn([
        "relative flex items-center justify-between",
        "rounded-xs px-2 py-1.5 text-sm outline-hidden",
        "cursor-pointer select-none",
        "hover:bg-accent hover:text-accent-foreground",
        "group",
      ])}
    >
      <span className="text-neutral-400">{label}</span>
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
            "rounded-full px-2 py-0.5 text-[11px] font-medium",
            "opacity-0 group-hover:opacity-100",
            "transition-all duration-150",
            isCloud
              ? "bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-xs hover:shadow-md"
              : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs hover:shadow-md",
          ])}
          onClick={handleAction}
        >
          {isCloud ? cloudButtonLabel : "Download"}
        </button>
      )}
    </div>
  );
}
