import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowSquareOut,
  CircleNotch,
  WarningCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type AnyFieldApi, useForm } from "@tanstack/react-form";
import { useMutation, useQueries } from "@tanstack/react-query";
import {
  Children,
  cloneElement,
  type ComponentType,
  type CSSProperties,
  isValidElement,
  type ReactNode,
  useMemo,
  useState,
} from "react";
import { Streamdown } from "streamdown";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import type { AIProvider } from "@anlg/store";
import { aiProviderSchema } from "@anlg/store";
import { markdownComponents } from "@anlg/ui/components/markdown";
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
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

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

export * from "./anarlog-cloud-button";
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
      {...stylex.props(styles.providerLogo)}
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
  sx,
}: {
  src: string;
  alt: string;
} & StyleXProps) {
  return (
    <img
      src={src}
      alt={alt}
      data-slot="provider-brand-icon"
      {...stylex.props(styles.providerBrandImage, sx)}
    />
  );
}

export function AiIconSlot({
  children,
  title,
  sx,
}: {
  children: ReactNode;
  title?: string;
} & StyleXProps) {
  return (
    <span
      title={title}
      aria-label={title}
      data-slot="ai-icon"
      {...stylex.props(styles.aiIcon, sx)}
    >
      <span data-slot="ai-icon-art" {...stylex.props(styles.aiIconArt)}>
        {styleArtwork(children, styles.aiIconArtwork)}
      </span>
    </span>
  );
}

export function ProviderIconSlot({ children }: { children: ReactNode }) {
  return <AiIconSlot>{children}</AiIconSlot>;
}

export function ProviderButtonIcon({ children }: { children: ReactNode }) {
  return (
    <span aria-hidden {...stylex.props(styles.buttonIcon)}>
      {styleArtwork(children, styles.buttonIconArtwork)}
    </span>
  );
}

function styleArtwork(children: ReactNode, sx: StyleXProps["sx"]) {
  return Children.map(children, (child) => {
    if (
      !isValidElement<{
        className?: string;
        style?: CSSProperties;
      }>(child)
    ) {
      return child;
    }

    return cloneElement(
      child,
      mergeStyleXProps(sx, child.props.className, child.props.style),
    );
  });
}

export function providerRowId(providerType: ProviderType, providerId: string) {
  return `${providerType}:${providerId}`;
}

export function useProviderAvailability(
  providerType: ProviderType,
  providers: readonly ProviderConfig[],
): Record<string, boolean | undefined> {
  const billing = useBillingAccess();
  const configuredProviders = useAiProviders(providerType);

  const inputs = providers
    .filter((provider) => provider.checkAvailability)
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
        apiKey,
      ],
      queryFn: () => provider.checkAvailability?.(baseUrl, apiKey) ?? false,
      enabled: isConfigured,
      retry: false,
      refetchInterval: 5_000,
    })),
  });

  const entries = inputs.map(
    ({ provider, isConfigured }, index) =>
      [
        provider.id,
        !isConfigured
          ? false
          : queries[index]?.isPending
            ? undefined
            : queries[index]?.data === true,
      ] as const,
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

  if (providerDef?.checkAvailability) {
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
    config.id,
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
      sx={styles.resetButton}
    >
      {clearProvider.isPending ? (
        <CircleNotch
          {...stylex.props(styles.smallSpinner)}
          aria-hidden="true"
        />
      ) : null}
      <Trans>Reset</Trans>
    </Button>
  ) : null;

  return (
    <AccordionItem
      disabled={config.disabled || locked}
      value={config.id}
      sx={[
        styles.providerCard,
        looksReady ? styles.providerCardReady : styles.providerCardIncomplete,
      ]}
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
        sx={[
          styles.providerTrigger,
          (config.disabled || locked) && styles.providerTriggerDisabled,
        ]}
      >
        <div {...stylex.props(styles.providerIdentity)}>
          <ProviderIconSlot>{config.icon}</ProviderIconSlot>
          <span>{config.displayName}</span>
          {config.badge && <ProviderBadge badge={config.badge} />}
        </div>
      </AccordionTrigger>
      <AccordionContent
        sx={[
          styles.providerContent,
          providerType === "llm" && styles.llmProviderContent,
        ]}
      >
        {providerContext}

        {isSubscription ? (
          <div {...stylex.props(styles.subscription)}>
            {hasStoredConfig ? (
              <p {...stylex.props(styles.mutedText)}>
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
          {...stylex.props(styles.form)}
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
            <div {...stylex.props(styles.subscriptionConnect)}>
              <div {...stylex.props(styles.divider)}>
                <div {...stylex.props(styles.dividerLine)} />
                <span {...stylex.props(styles.mutedText)}>
                  <Trans>or</Trans>
                </span>
                <div {...stylex.props(styles.dividerLine)} />
              </div>
              {subscriptionReady ? (
                <div {...stylex.props(styles.subscriptionStatus)}>
                  <p {...stylex.props(styles.mutedText)}>
                    {t`Connected with your ${subscriptionProvider.displayName} subscription.`}
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => void handleResetSubscription()}
                    disabled={clearSubscription.isPending}
                    sx={[styles.resetButton, styles.nonShrinking]}
                  >
                    {clearSubscription.isPending ? (
                      <CircleNotch
                        {...stylex.props(styles.smallSpinner)}
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
                  sx={styles.selfStart}
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
            <div {...stylex.props(styles.links)}>
              {config.links.download && (
                <a
                  href={config.links.download.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  {...stylex.props(styles.link)}
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
                  {...stylex.props(styles.link)}
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
                  {...stylex.props(styles.link)}
                >
                  {config.links.setup.label}
                  <ArrowSquareOut size={12} />
                </a>
              )}
            </div>
          )}
          {showAdvanced ? (
            <details>
              <summary {...stylex.props(styles.summary)}>
                <Trans>Advanced</Trans>
              </summary>
              <div {...stylex.props(styles.advancedFields)}>
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
            <p {...stylex.props(styles.errorText)}>
              {clearProvider.error.message}
            </p>
          )}
          {providerMutation.error &&
            !isKeychainAccessError(providerMutation.error) && (
              <p {...stylex.props(styles.errorText)}>
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
      {...stylex.props(
        styles.badge,
        isBatchOnly ? styles.batchBadge : styles.standardBadge,
      )}
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
      <TooltipContent side="top" sx={styles.badgeTooltip}>
        <Trans>
          Runs after the recording finishes, not during the meeting.
        </Trans>
      </TooltipContent>
    </Tooltip>
  );
}

const streamdownComponents = {
  ...markdownComponents,
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => {
    return (
      <ul {...stylex.props(styles.unorderedList)}>
        {props.children as React.ReactNode}
      </ul>
    );
  },
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => {
    return (
      <ol {...stylex.props(styles.orderedList)}>
        {props.children as React.ReactNode}
      </ol>
    );
  },
  li: (props: React.HTMLAttributes<HTMLLIElement>) => {
    return (
      <li {...stylex.props(styles.markdownBlock)}>
        {props.children as React.ReactNode}
      </li>
    );
  },
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => {
    return (
      <p {...stylex.props(styles.markdownBlock)}>
        {props.children as React.ReactNode}
      </p>
    );
  },
  a: ({
    children,
    className,
    style,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement>) => {
    return (
      <a
        {...props}
        {...mergeStyleXProps(styles.markdownLink, className, style)}
      >
        {children as React.ReactNode}
      </a>
    );
  },
} as const;

export function StyledStreamdown({
  children,
  sx,
}: {
  children: string;
} & StyleXProps) {
  return (
    <Streamdown
      components={streamdownComponents}
      {...stylex.props(styles.streamdown, sx)}
      controls={false}
      isAnimating={false}
    >
      {children}
    </Streamdown>
  );
}

function useProvider(providerType: ProviderType, id: string) {
  const { providers, isReady } = useAiProvidersState(providerType);
  const providerRow = providers[providerRowId(providerType, id)];
  const providerMutation = useSetAiProvider(providerType, id);

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
    <div {...stylex.props(styles.field)}>
      <label {...stylex.props(styles.fieldLabel)}>{label}</label>
      <InputGroup sx={styles.inputGroup}>
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
        <p {...stylex.props(styles.fieldError)}>
          <WarningCircle
            {...stylex.props(styles.warningIcon)}
            aria-hidden="true"
          />
          <span>{errorMessage}</span>
        </p>
      )}
    </div>
  );
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  advancedFields: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    marginTop: "0.5rem",
  },
  aiIcon: {
    alignItems: "center",
    color: colors.foreground,
    display: "flex",
    flexShrink: 0,
    height: "1.25rem",
    justifyContent: "center",
    overflow: "hidden",
    width: "1.25rem",
  },
  aiIconArt: {
    alignItems: "center",
    display: "flex",
    height: "100%",
    justifyContent: "center",
    overflow: "hidden",
    width: "100%",
  },
  aiIconArtwork: {
    color: "inherit",
    display: "block",
    height: "100%",
    width: "100%",
  },
  badge: {
    color: colors.mutedForeground,
    textTransform: "none",
  },
  badgeTooltip: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
    maxWidth: "16rem",
  },
  batchBadge: {
    backgroundColor: `color-mix(in oklab, ${colors.background} 40%, transparent)`,
    borderRadius: radii.md,
    cursor: "help",
    fontSize: "11px",
    fontWeight: 500,
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  buttonIcon: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "0.875rem",
    justifyContent: "center",
    overflow: "hidden",
    width: "0.875rem",
  },
  buttonIconArtwork: {
    display: "block",
    height: "100%",
    width: "100%",
  },
  divider: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
  },
  dividerLine: {
    backgroundColor: colors.border,
    flex: "1",
    height: "1px",
  },
  errorText: {
    color: colors.destructive,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  field: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  fieldError: {
    alignItems: "center",
    color: colors.destructive,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.375rem",
    lineHeight: "1rem",
  },
  fieldLabel: {
    display: "block",
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  inputGroup: {
    backgroundColor: colors.card,
  },
  link: {
    alignItems: "center",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "inline-flex",
    gap: "0.125rem",
    textDecorationLine: {
      default: "none",
      ":hover": "underline",
    },
  },
  links: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.75rem",
    gap: "1rem",
    lineHeight: "1rem",
  },
  llmProviderContent: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  markdownBlock: {
    marginBottom: "0.25rem",
  },
  markdownLink: {
    color: colors.foreground,
    fontWeight: 500,
    textDecorationColor: {
      default: `color-mix(in oklab, ${colors.foreground} 50%, transparent)`,
      ":hover": colors.foreground,
    },
    textDecorationLine: "underline",
    textUnderlineOffset: "2px",
  },
  mutedText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  nonShrinking: {
    flexShrink: 0,
  },
  orderedList: {
    display: "block",
    listStyleType: "decimal",
    marginBottom: "0.25rem",
    paddingLeft: "1.5rem",
    position: "relative",
  },
  providerBrandImage: {
    filter: "var(--provider-brand-filter)",
    objectFit: "contain",
    objectPosition: "center",
  },
  providerCard: {
    backgroundColor: colors.muted,
    borderRadius: "22px",
    borderWidth: "2px",
  },
  providerCardIncomplete: {
    borderStyle: "dashed",
  },
  providerCardReady: {
    borderColor: colors.border,
    borderStyle: "solid",
  },
  providerContent: {
    paddingInline: "1rem",
  },
  providerIdentity: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  providerLogo: {
    height: "100%",
    objectFit: "contain",
    objectPosition: "center",
    width: "100%",
  },
  providerTrigger: {
    gap: "0.5rem",
    paddingInline: "1rem",
    textDecorationLine: {
      default: "none",
      ":hover": "none",
    },
    textTransform: "capitalize",
  },
  providerTriggerDisabled: {
    color: colors.mutedForeground,
    cursor: "not-allowed",
  },
  resetButton: {
    alignSelf: "flex-start",
    backgroundColor: {
      default: null,
      ":hover": "transparent",
    },
    color: {
      default: colors.destructive,
      ":hover": `color-mix(in oklab, ${colors.destructive} 80%, transparent)`,
    },
    height: "1.75rem",
    paddingInline: 0,
  },
  selfStart: {
    alignSelf: "flex-start",
  },
  smallSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.75rem",
    width: "0.75rem",
  },
  standardBadge: {
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    fontSize: "0.75rem",
    fontWeight: 300,
    lineHeight: "1rem",
    paddingInline: "0.5rem",
  },
  streamdown: {
    fontSize: "0.875rem",
    marginTop: "0.25rem",
  },
  subscription: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.75rem",
  },
  subscriptionConnect: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  subscriptionStatus: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
  },
  summary: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    cursor: "pointer",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    textDecorationLine: {
      default: "none",
      ":hover": "underline",
    },
  },
  unorderedList: {
    display: "block",
    listStyleType: "disc",
    marginBottom: "0.25rem",
    paddingLeft: "1.5rem",
    position: "relative",
  },
  warningIcon: {
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
});
