import { useForm } from "@tanstack/react-form";
import { useQueries, useQuery } from "@tanstack/react-query";
import { arch } from "@tauri-apps/plugin-os";
import { Check, Loader2 } from "lucide-react";

import { commands as listenerCommands } from "@hypr/plugin-listener";
import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";
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

  const languageSupport = useQuery({
    queryKey: [
      "stt-language-support",
      current_stt_provider,
      current_stt_model,
      spoken_languages,
    ],
    queryFn: async () => {
      const result = await listenerCommands.isSupportedLanguagesLive(
        current_stt_provider!,
        current_stt_model ?? null,
        spoken_languages ?? [],
      );
      return result.status === "ok" ? result.data : true;
    },
    enabled: !!(current_stt_provider && spoken_languages?.length),
  });

  const hasLanguageWarning =
    isConfigured && languageSupport.data === false && !hasError;

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

  const form = useForm({
    defaultValues: {
      provider: current_stt_provider || "",
      model: current_stt_model || "",
    },
    listeners: {
      onChange: ({ formApi }) => {
        const {
          form: { errors },
        } = formApi.getAllErrors();
        if (errors.length > 0) {
          console.log(errors);
        }

        void formApi.handleSubmit();
      },
    },
    onSubmit: ({ value }) => {
      handleSelectProvider(value.provider);
      handleSelectModel(value.model);
    },
  });

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
          <form.Field
            name="provider"
            listeners={{
              onChange: () => {
                form.setFieldValue("model", "");
              },
            }}
          >
            {(field) => (
              <div className="min-w-0 flex-2" data-stt-provider-selector>
                <Select
                  value={field.state.value}
                  onValueChange={(value) => field.handleChange(value)}
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
                        const locked = requiresPro && !billing.isPro;
                        return (
                          <SelectItem
                            key={provider.id}
                            value={provider.id}
                            disabled={
                              provider.disabled || !configured || locked
                            }
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
            )}
          </form.Field>

          <span className="text-neutral-500">/</span>

          <form.Field name="model">
            {(field) => {
              const providerId = field.form.getFieldValue(
                "provider",
              ) as ProviderId;
              if (providerId === "custom") {
                return (
                  <div className="min-w-0 flex-3">
                    <Input
                      value={field.state.value}
                      onChange={(event) =>
                        field.handleChange(event.target.value)
                      }
                      className="text-xs"
                      placeholder="Enter a model identifier"
                    />
                  </div>
                );
              }

              const models = configuredProviders?.[providerId]?.models ?? [];

              return (
                <div className="min-w-0 flex-3">
                  <Select
                    value={field.state.value}
                    onValueChange={(value) => field.handleChange(value)}
                    disabled={models.length === 0}
                  >
                    <SelectTrigger
                      className={cn([
                        "bg-white shadow-none focus:ring-0",
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
                      {models.map((model) => (
                        <ModelSelectItem
                          key={model.id}
                          model={model}
                          onDownload={() =>
                            startDownload(model.id as LocalModel)
                          }
                          onStartTrial={startTrial}
                        />
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              );
            }}
          </form.Field>
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

type ModelEntry = { id: string; isDownloaded: boolean; displayName?: string };

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

  const [p2, p3, whisperLargeV3, ...cactusDownloaded] = useQueries({
    queries: [
      sttModelQueries.isDownloaded("am-parakeet-v2"),
      sttModelQueries.isDownloaded("am-parakeet-v3"),
      sttModelQueries.isDownloaded("am-whisper-large-v3"),
      ...cactusModels.map((m) => sttModelQueries.isDownloaded(m.key)),
    ],
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
          isPro: billing.isPro,
          config: { base_url: baseUrl, api_key: apiKey },
        }).length === 0;

      if (!eligible) {
        return [provider.id, { configured: false, models: [] }];
      }

      if (provider.id === "hyprnote") {
        const models: ModelEntry[] = [
          { id: "cloud", isDownloaded: billing.isPro },
        ];

        if (isAppleSilicon) {
          cactusModels.forEach((model, i) => {
            models.push({
              id: model.key,
              isDownloaded: cactusDownloaded[i]?.data ?? false,
              displayName: model.display_name,
            });
          });

          models.push(
            {
              id: "am-parakeet-v2",
              isDownloaded: p2.data ?? false,
            },
            {
              id: "am-parakeet-v3",
              isDownloaded: p3.data ?? false,
            },
            {
              id: "am-whisper-large-v3",
              isDownloaded: whisperLargeV3.data ?? false,
            },
          );
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
