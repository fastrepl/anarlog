import { Trans, useLingui } from "@lingui/react/macro";
import { type AnyFieldApi, useForm } from "@tanstack/react-form";
import { useMutation, useQueries } from "@tanstack/react-query";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { type ComponentType, type ReactNode, useMemo, useState } from "react";
import { Streamdown } from "streamdown";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import {
  ProviderCredentialError,
  providerCredentialIdentity,
  verifyProviderCredentials,
} from "@anlg/provider-validation";
import type { AIProvider } from "@anlg/store";
import { aiProviderSchema } from "@anlg/store";
import {
  ArrowSquareOut,
  CircleNotch,
  WarningCircle,
} from "@anlg/ui/components/icons";
import { streamdownIcons } from "@anlg/ui/components/streamdown-icons";
import {
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@anlg/ui/components/ui/accordion";
import { Button } from "@anlg/ui/components/ui/button";
import {
  InputGroup,
  InputGroupInput,
} from "@anlg/ui/components/ui/input-group";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";
import { cn } from "@anlg/utils";

import {
  getProviderSelectionBlockers,
  getRequiredConfigFields,
  type ProviderRequirement,
  requiresEntitlement,
} from "./eligibility";
import { useProviderSelectionPrompt } from "./provider-selection-prompt";

import { useBillingAccess } from "~/auth/billing-context";
import {
  isKeychainAccessError,
  repairKeychainAccess,
  useAiProviders,
  useAiProvidersState,
  useClearAiProvider,
  useSetAiProvider,
} from "~/settings/providers";
import { setSettingValues } from "~/settings/queries";
import { SettingsAlertToast } from "~/shared/ui/settings-alert";

export * from "./model-combobox";
export * from "./provider-search";

type ProviderType = "stt" | "llm";

type ProviderConfig = {
  id: string;
  displayName: string;
  icon: ReactNode;
  badge?: string | null;
  baseUrl?: string;
  authKind?: "api" | "subscription";
  disabled?: boolean;
  requirements: ProviderRequirement[];
  checkAvailability?: (baseUrl: string, apiKey: string) => Promise<boolean>;
  hideAdvanced?: boolean;
  links?: {
    download?: { label: string; url: string };
    models?: { label: string; url: string };
    setup?: { label: string; url: string };
  };
};

const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

export function AnarlogProviderIcon() {
  return (
    <img
      src={ANARLOG_ICON_SRC}
      alt="Anarlog"
      data-slot="provider-logo"
      className="size-full object-contain object-center"
    />
  );
}

type LobeIconComponent = ComponentType<{
  color?: string;
  size?: number | string;
}> & {
  Color?: ComponentType<{ size?: number | string }>;
  colorPrimary?: string;
};

const THEME_TINTED_BRAND_COLORS = new Set([
  "#000",
  "#000000",
  "#fff",
  "#ffffff",
  "#141413",
  "#16191e",
  "#f1f0e8",
]);

export function ProviderLobeIcon({ icon: Icon }: { icon: LobeIconComponent }) {
  if (Icon.Color) {
    return <Icon.Color />;
  }

  const brandColor = Icon.colorPrimary?.toLowerCase();
  if (brandColor && !THEME_TINTED_BRAND_COLORS.has(brandColor)) {
    return <Icon color={Icon.colorPrimary} />;
  }

  return <Icon />;
}

export function ProviderBrandImage({
  src,
  alt,
  className,
}: {
  src: string;
  alt: string;
  className?: string;
}) {
  return (
    <img
      src={src}
      alt={alt}
      data-slot="provider-brand-icon"
      className={cn([
        "object-contain object-center [filter:var(--provider-brand-filter)]",
        className,
      ])}
    />
  );
}

export function AiIconSlot({
  children,
  title,
  className,
}: {
  children: ReactNode;
  title?: string;
  className?: string;
}) {
  return (
    <span
      title={title}
      aria-label={title}
      data-slot="ai-icon"
      className={cn([
        "text-foreground flex size-5 shrink-0 items-center justify-center overflow-hidden",
        "[&_[data-slot=provider-brand-icon]]:[filter:var(--provider-brand-filter)]",
        className,
      ])}
    >
      <span
        data-slot="ai-icon-art"
        className={cn([
          "flex size-full items-center justify-center overflow-hidden",
          "[&>img]:block [&>img]:size-full [&>svg]:block [&>svg]:size-full [&>svg]:text-inherit",
        ])}
      >
        {children}
      </span>
    </span>
  );
}

export function ProviderIconSlot({ children }: { children: ReactNode }) {
  return <AiIconSlot>{children}</AiIconSlot>;
}

export function ProviderButtonIcon({ children }: { children: ReactNode }) {
  return (
    <span
      aria-hidden
      className="flex size-3.5 shrink-0 items-center justify-center overflow-hidden [&>img]:block [&>img]:size-full [&>svg]:block [&>svg]:size-full"
    >
      {children}
    </span>
  );
}

export function providerRowId(providerType: ProviderType, providerId: string) {
  return `${providerType}:${providerId}`;
}

export function requiresKeyVerification(
  provider: Pick<
    ProviderConfig,
    "requirements" | "authKind" | "checkAvailability"
  >,
) {
  return (
    provider.authKind !== "subscription" &&
    !provider.checkAvailability &&
    getRequiredConfigFields(provider.requirements).includes("api_key")
  );
}

export function useProviderAvailability(
  providerType: ProviderType,
  providers: readonly ProviderConfig[],
): Record<string, boolean | undefined> {
  const billing = useBillingAccess();
  const configuredProviders = useAiProviders(providerType);

  const inputs = providers
    .filter(
      (provider) =>
        provider.checkAvailability || requiresKeyVerification(provider),
    )
    .map((provider) => {
      const config =
        configuredProviders[providerRowId(providerType, provider.id)];
      const baseUrl = String(config?.base_url || provider.baseUrl || "").trim();
      const apiKey = String(config?.api_key || "").trim();
      const isConfigured =
        getProviderSelectionBlockers(provider.requirements, {
          isAuthenticated: true,
          isPaid: billing.isPaid,
          config: { base_url: baseUrl, api_key: apiKey },
        }).length === 0;

      return { provider, baseUrl, apiKey, isConfigured };
    });

  const queries = useQueries({
    queries: inputs.map(({ provider, baseUrl, apiKey, isConfigured }) => ({
      queryKey: [
        "ai-provider-availability",
        providerType,
        provider.id,
        baseUrl,
        providerCredentialIdentity({ provider: provider.id, baseUrl, apiKey }),
      ],
      queryFn: async ({ signal }: { signal: AbortSignal }) => {
        if (provider.checkAvailability)
          return provider.checkAvailability(baseUrl, apiKey);
        try {
          await verifyProviderCredentials(
            { provider: provider.id, baseUrl, apiKey },
            tauriFetch,
            signal,
          );
        } catch (error) {
          if (error instanceof ProviderCredentialError && !error.retryable)
            return false;
          throw error;
        }
        return true;
      },
      enabled: isConfigured,
      retry: false,
      staleTime: provider.checkAvailability ? 0 : 5 * 60_000,
      gcTime: 0,
      refetchInterval: provider.checkAvailability ? 5_000 : (false as const),
    })),
  });

  const entries = inputs.map(
    ({ provider, isConfigured }, index) =>
      [provider.id, !isConfigured ? false : queries[index]?.data] as const,
  );

  // Callers put this record in memo dependency lists, so its identity has to
  // stay stable while the values do; a fresh object each render would recompute
  // those memos and churn the derived listModels closures used as query keys.
  const signature = entries.map(([id, value]) => `${id}=${value}`).join("|");

  return useMemo(() => Object.fromEntries(entries), [signature]);
}

export function useIsProviderReady(
  providerId: string,
  providerType: ProviderType,
  providers: readonly ProviderConfig[],
) {
  const billing = useBillingAccess();
  const configuredProviders = useAiProviders(providerType);
  const availability = useProviderAvailability(providerType, providers);
  const providerDef = providers.find((p) => p.id === providerId);

  if (
    providerDef &&
    (providerDef.checkAvailability || requiresKeyVerification(providerDef))
  ) {
    return availability[providerId];
  }

  const config = configuredProviders[providerRowId(providerType, providerId)];
  const baseUrl = String(config?.base_url || providerDef?.baseUrl || "").trim();
  const apiKey = String(config?.api_key || "").trim();

  return (
    !!providerDef &&
    getProviderSelectionBlockers(providerDef.requirements, {
      isAuthenticated: true,
      isPaid: billing.isPaid,
      config: { base_url: baseUrl, api_key: apiKey },
    }).length === 0
  );
}

export function NonAnarlogProviderCard({
  config,
  providerType,
  providers,
  providerContext,
  currentProvider,
  onConnect,
  onConnectSubscription,
  subscriptionProviderId,
}: {
  config: ProviderConfig;
  providerType: ProviderType;
  providers: readonly ProviderConfig[];
  providerContext?: ReactNode;
  currentProvider?: string;
  onConnect?: () => void;
  onConnectSubscription?: () => void;
  subscriptionProviderId?: string;
}) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const [provider, providerMutation, providerStateReady] = useProvider(
    providerType,
    config,
  );
  const clearProvider = useClearAiProvider(providerType, config.id);
  const clearSubscription = useClearAiProvider(
    providerType,
    subscriptionProviderId ?? config.id,
  );
  const subscriptionProvider = providers.find(
    (provider) => provider.id === subscriptionProviderId,
  );
  const [hasUnresolvedKeychainError, setHasUnresolvedKeychainError] =
    useState(false);
  const [isKeychainRecoveryInProgress, setIsKeychainRecoveryInProgress] =
    useState(false);
  const locked =
    requiresEntitlement(config.requirements, "pro") && !billing.isPaid;
  const isReady = useIsProviderReady(config.id, providerType, providers);
  const configuredProviders = useAiProviders(providerType);
  const subscriptionReady = Boolean(
    subscriptionProviderId &&
    configuredProviders[
      providerRowId(providerType, subscriptionProviderId)
    ]?.api_key?.trim(),
  );
  const looksReady = isReady || subscriptionReady;

  const requiredFields = getRequiredConfigFields(config.requirements);
  const isSubscription = config.authKind === "subscription";
  const showApiKey = requiredFields.includes("api_key") && !isSubscription;
  const showBaseUrl = requiredFields.includes("base_url") && !isSubscription;
  const notifyProviderSelection = useProviderSelectionPrompt({
    providerType,
    providerId: config.id,
    providerName: config.displayName,
    currentProvider,
    providerStateReady,
    storedApiKey: provider?.api_key,
  });

  const form = useForm({
    onSubmit: async ({ value }) => {
      try {
        await providerMutation.mutateAsync(value);
      } catch (error) {
        if (isKeychainAccessError(error)) {
          setHasUnresolvedKeychainError(true);
        }
        return;
      }

      setHasUnresolvedKeychainError(false);
      notifyProviderSelection(value.api_key);

      void analyticsCommands.event({
        event: "ai_provider_configured",
        provider: value.type,
      });
      void analyticsCommands.setProperties({
        set: {
          has_configured_ai: true,
        },
      });
    },
    defaultValues:
      provider ??
      ({
        type: providerType,
        base_url: config.baseUrl ?? "",
        api_key: "",
      } satisfies AIProvider),
    listeners: {
      onChangeDebounceMs: 500,
      onChange: ({ formApi }) => {
        providerMutation.reset();
        queueMicrotask(() => {
          void formApi.handleSubmit();
        });
      },
    },
    validators: { onChange: aiProviderSchema },
  });
  const repairMutation = useMutation<void, Error>({
    mutationFn: repairKeychainAccess,
    onMutate: () => {
      setIsKeychainRecoveryInProgress(true);
    },
    onSuccess: async () => {
      await form.handleSubmit();
    },
    onSettled: () => {
      setIsKeychainRecoveryInProgress(false);
    },
  });
  const keychainToastDescription = isKeychainRecoveryInProgress
    ? t`Unlock your login Keychain in the macOS prompt. Anarlog will retry saving this API key automatically.`
    : (repairMutation.error?.message ??
      t`macOS cannot access your login Keychain. Repairing briefly locks it and asks for your Mac password before Anarlog retries this API key.`);
  const hasStoredConfig =
    Boolean(provider?.api_key?.trim()) ||
    Boolean(
      provider?.base_url?.trim() &&
      provider.base_url.trim() !== (config.baseUrl ?? "").trim(),
    );

  const handleResetSubscription = async () => {
    if (!subscriptionProviderId || clearSubscription.isPending) {
      return;
    }

    try {
      await clearSubscription.mutateAsync();

      if (currentProvider === subscriptionProviderId) {
        await setSettingValues(
          providerType === "llm"
            ? { current_llm_provider: "", current_llm_model: "" }
            : { current_stt_provider: "", current_stt_model: "" },
        );
      }
    } catch {
      return;
    }
  };

  const handleReset = async () => {
    if (clearProvider.isPending) {
      return;
    }

    try {
      await clearProvider.mutateAsync();

      if (currentProvider === config.id) {
        await setSettingValues(
          providerType === "llm"
            ? { current_llm_provider: "", current_llm_model: "" }
            : { current_stt_provider: "", current_stt_model: "" },
        );
      }

      form.reset({
        type: providerType,
        base_url: config.baseUrl ?? "",
        api_key: "",
      });
      setHasUnresolvedKeychainError(false);
      providerMutation.reset();
    } catch {
      return;
    }
  };

  const hasAdvancedFields = (!showBaseUrl && !!config.baseUrl) || !showApiKey;
  const showAdvanced = !config.hideAdvanced && hasAdvancedFields;
  const resetAction = hasStoredConfig ? (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={() => void handleReset()}
      disabled={clearProvider.isPending}
      className="text-destructive hover:text-destructive/80 h-7 self-start px-0 hover:bg-transparent"
    >
      {clearProvider.isPending ? (
        <CircleNotch className="size-3 animate-spin" aria-hidden="true" />
      ) : null}
      <Trans>Reset</Trans>
    </Button>
  ) : null;

  return (
    <AccordionItem
      disabled={config.disabled || locked}
      value={config.id}
      className={cn([
        "bg-muted rounded-[22px] border-2",
        looksReady ? "border-border border-solid" : "border-dashed",
      ])}
    >
      <SettingsAlertToast
        id={`provider-keychain-access:${providerType}:${config.id}`}
        description={
          hasUnresolvedKeychainError ? keychainToastDescription : undefined
        }
        variant="error"
        lifecycle="condition-bound"
        action={
          isKeychainRecoveryInProgress
            ? undefined
            : {
                label: t`Repair Keychain Access`,
                onClick: () => repairMutation.mutate(),
              }
        }
      />
      <AccordionTrigger
        className={cn([
          "gap-2 px-4 capitalize hover:no-underline",
          (config.disabled || locked) &&
            "text-muted-foreground cursor-not-allowed",
        ])}
      >
        <div className="flex items-center gap-2">
          <ProviderIconSlot>{config.icon}</ProviderIconSlot>
          <span>{config.displayName}</span>
          {config.badge && <ProviderBadge badge={config.badge} />}
        </div>
      </AccordionTrigger>
      <AccordionContent
        className={cn([
          "px-4",
          providerType === "llm" && "flex flex-col gap-3",
        ])}
      >
        {providerContext}

        {isSubscription ? (
          <div className="mb-3 flex items-center gap-2">
            {hasStoredConfig ? (
              <p className="text-muted-foreground text-xs">
                <Trans>Connected with your existing subscription.</Trans>
              </p>
            ) : (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={onConnect}
              >
                <ProviderButtonIcon>{config.icon}</ProviderButtonIcon>
                {t`Connect ${config.displayName}`}
              </Button>
            )}
          </div>
        ) : null}

        <form
          className="flex flex-col gap-4"
          onSubmit={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
        >
          {showBaseUrl && (
            <form.Field name="base_url">
              {(field) => <FormField field={field} label={t`Base URL`} />}
            </form.Field>
          )}
          {showApiKey && (
            <form.Field name="api_key">
              {(field) => (
                <FormField
                  field={field}
                  label={t`API Key`}
                  placeholder={t`Enter your API key`}
                  type="password"
                />
              )}
            </form.Field>
          )}
          {subscriptionProvider && onConnectSubscription ? (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-3">
                <div className="bg-border h-px flex-1" />
                <span className="text-muted-foreground text-xs">
                  <Trans>or</Trans>
                </span>
                <div className="bg-border h-px flex-1" />
              </div>
              {subscriptionReady ? (
                <div className="flex items-center justify-between gap-2">
                  <p className="text-muted-foreground text-xs">
                    {t`Connected with your ${subscriptionProvider.displayName} subscription.`}
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => void handleResetSubscription()}
                    disabled={clearSubscription.isPending}
                    className="text-destructive hover:text-destructive/80 h-7 shrink-0 px-0 hover:bg-transparent"
                  >
                    {clearSubscription.isPending ? (
                      <CircleNotch
                        className="size-3 animate-spin"
                        aria-hidden="true"
                      />
                    ) : null}
                    <Trans>Reset</Trans>
                  </Button>
                </div>
              ) : (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={onConnectSubscription}
                  className="self-start"
                >
                  <ProviderButtonIcon>
                    {subscriptionProvider.icon}
                  </ProviderButtonIcon>
                  {t`Connect ${subscriptionProvider.displayName}`}
                </Button>
              )}
            </div>
          ) : null}
          {config.links && (
            <div className="flex items-center gap-4 text-xs">
              {config.links.download && (
                <a
                  href={config.links.download.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-muted-foreground hover:text-foreground inline-flex items-center gap-0.5 hover:underline"
                >
                  {config.links.download.label}
                  <ArrowSquareOut size={12} />
                </a>
              )}
              {config.links.models && (
                <a
                  href={config.links.models.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-muted-foreground hover:text-foreground inline-flex items-center gap-0.5 hover:underline"
                >
                  {config.links.models.label}
                  <ArrowSquareOut size={12} />
                </a>
              )}
              {config.links.setup && (
                <a
                  href={config.links.setup.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-muted-foreground hover:text-foreground inline-flex items-center gap-0.5 hover:underline"
                >
                  {config.links.setup.label}
                  <ArrowSquareOut size={12} />
                </a>
              )}
            </div>
          )}
          {showAdvanced ? (
            <details>
              <summary className="text-muted-foreground hover:text-foreground cursor-pointer text-xs hover:underline">
                <Trans>Advanced</Trans>
              </summary>
              <div className="mt-2 flex flex-col gap-4">
                {!showBaseUrl && config.baseUrl && (
                  <form.Field name="base_url">
                    {(field) => <FormField field={field} label={t`Base URL`} />}
                  </form.Field>
                )}
                {!showApiKey && (
                  <form.Field name="api_key">
                    {(field) => (
                      <FormField
                        field={field}
                        label={t`API Key`}
                        placeholder={t`Enter your API key (optional)`}
                        type="password"
                      />
                    )}
                  </form.Field>
                )}
                {resetAction}
              </div>
            </details>
          ) : (
            resetAction
          )}
          {clearProvider.error && (
            <p className="text-destructive text-xs">
              {clearProvider.error.message}
            </p>
          )}
          {providerMutation.error &&
            !isKeychainAccessError(providerMutation.error) && (
              <p className="text-destructive text-xs">
                {providerMutation.error.message}
              </p>
            )}
        </form>
      </AccordionContent>
    </AccordionItem>
  );
}

function ProviderBadge({ badge }: { badge: string }) {
  const isBatchOnly = badge === "Batch only";
  const badgeNode = (
    <span
      className={cn([
        "text-muted-foreground normal-case",
        isBatchOnly
          ? "bg-background/40 cursor-help rounded-md px-1.5 py-0.5 text-[11px] font-medium"
          : "border-border rounded-full border px-2 text-xs font-light",
      ])}
    >
      {badge}
    </span>
  );

  if (!isBatchOnly) {
    return badgeNode;
  }

  return (
    <Tooltip delayDuration={100}>
      <TooltipTrigger asChild>{badgeNode}</TooltipTrigger>
      <TooltipContent side="top" className="max-w-64 text-xs">
        <Trans>
          Runs after the recording finishes, not during the meeting.
        </Trans>
      </TooltipContent>
    </Tooltip>
  );
}

const streamdownComponents = {
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => {
    return (
      <ul className="relative mb-1 block list-disc pl-6">
        {props.children as React.ReactNode}
      </ul>
    );
  },
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => {
    return (
      <ol className="relative mb-1 block list-decimal pl-6">
        {props.children as React.ReactNode}
      </ol>
    );
  },
  li: (props: React.HTMLAttributes<HTMLLIElement>) => {
    return <li className="mb-1">{props.children as React.ReactNode}</li>;
  },
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => {
    return <p className="mb-1">{props.children as React.ReactNode}</p>;
  },
  a: ({
    children,
    className,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement>) => {
    return (
      <a
        {...props}
        className={cn([
          "text-foreground font-medium underline underline-offset-2",
          "decoration-foreground/50 hover:decoration-foreground",
          className,
        ])}
      >
        {children as React.ReactNode}
      </a>
    );
  },
} as const;

export function StyledStreamdown({
  children,
  className,
}: {
  children: string;
  className?: string;
}) {
  return (
    <Streamdown
      icons={streamdownIcons}
      components={streamdownComponents}
      className={cn(["mt-1 text-sm", className])}
      isAnimating={false}
    >
      {children}
    </Streamdown>
  );
}

function useProvider(providerType: ProviderType, config: ProviderConfig) {
  const { providers, isReady } = useAiProvidersState(providerType);
  const providerRow = providers[providerRowId(providerType, config.id)];
  const providerMutation = useSetAiProvider(providerType, config.id, {
    verifyCredentials: requiresKeyVerification(config),
    defaultBaseUrl: config.baseUrl,
  });

  const { data } = aiProviderSchema.safeParse(providerRow);
  return [data, providerMutation, isReady] as const;
}

function FormField({
  field,
  label,
  placeholder,
  type,
}: {
  field: AnyFieldApi;
  label: string;
  placeholder?: string;
  type?: string;
}) {
  const {
    meta: { errors, isTouched },
  } = field.state;
  const hasError = isTouched && errors && errors.length > 0;
  const errorMessage = hasError
    ? typeof errors[0] === "string"
      ? errors[0]
      : "message" in errors[0]
        ? errors[0].message
        : JSON.stringify(errors[0])
    : null;

  return (
    <div className="flex flex-col gap-2">
      <label className="block text-xs font-medium">{label}</label>
      <InputGroup className="bg-card">
        <InputGroupInput
          name={field.name}
          type={type}
          value={field.state.value}
          onChange={(e) => field.handleChange(e.target.value)}
          placeholder={placeholder}
          aria-invalid={hasError}
        />
      </InputGroup>
      {errorMessage && (
        <p className="text-destructive flex items-center gap-1.5 text-xs">
          <WarningCircle className="size-3.5 shrink-0" aria-hidden="true" />
          <span>{errorMessage}</span>
        </p>
      )}
    </div>
  );
}
