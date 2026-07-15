import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import { type AnyFieldApi, useForm } from "@tanstack/react-form";
import { ExternalLink } from "lucide-react";
import type { ReactNode } from "react";
import { Streamdown } from "streamdown";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type { AIProvider } from "@hypr/store";
import { aiProviderSchema } from "@hypr/store";
import {
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import {
  InputGroup,
  InputGroupInput,
} from "@hypr/ui/components/ui/input-group";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import {
  getProviderSelectionBlockers,
  getRequiredConfigFields,
  type ProviderRequirement,
  requiresEntitlement,
} from "./eligibility";

import { useBillingAccess } from "~/auth/billing";
import {
  useAiProvider,
  useAiProviders,
  useSetAiProvider,
} from "~/settings/providers";

export * from "./hypr-cloud-button";
export * from "./model-combobox";

type ProviderType = "stt" | "llm";

type ProviderConfig = {
  id: string;
  displayName: string;
  icon: ReactNode;
  badge?: string | null;
  baseUrl?: string;
  disabled?: boolean;
  requirements: ProviderRequirement[];
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
      className="size-4 object-contain object-center [clip-path:inset(6%_round_18%)]"
    />
  );
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

export function ProviderIconSlot({ children }: { children: ReactNode }) {
  return (
    <span
      data-slot="provider-icon"
      className={cn([
        "text-foreground flex size-5 shrink-0 items-center justify-center",
        "[&_svg]:block [&_svg]:size-full [&_svg]:text-inherit",
        "[&_iconify-icon]:text-inherit",
        "[&_[data-slot=provider-brand-icon]]:[filter:var(--provider-brand-filter)]",
      ])}
    >
      {children}
    </span>
  );
}

export function providerRowId(providerType: ProviderType, providerId: string) {
  return `${providerType}:${providerId}`;
}

function useIsProviderConfigured(
  providerId: string,
  providerType: ProviderType,
  providers: readonly ProviderConfig[],
) {
  const billing = useBillingAccess();
  const configuredProviders = useAiProviders(providerType);
  const providerDef = providers.find((p) => p.id === providerId);
  const config = configuredProviders[providerRowId(providerType, providerId)];

  if (!providerDef) {
    return false;
  }

  const baseUrl = String(config?.base_url || providerDef.baseUrl || "").trim();
  const apiKey = String(config?.api_key || "").trim();

  return (
    getProviderSelectionBlockers(providerDef.requirements, {
      isAuthenticated: true,
      isPaid: billing.isPaid,
      config: { base_url: baseUrl, api_key: apiKey },
    }).length === 0
  );
}

export function NonHyprProviderCard({
  config,
  providerType,
  providers,
  providerContext,
}: {
  config: ProviderConfig;
  providerType: ProviderType;
  providers: readonly ProviderConfig[];
  providerContext?: ReactNode;
}) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const [provider, providerMutation] = useProvider(providerType, config.id);
  const locked =
    requiresEntitlement(config.requirements, "pro") && !billing.isPaid;
  const isConfigured = useIsProviderConfigured(
    config.id,
    providerType,
    providers,
  );

  const requiredFields = getRequiredConfigFields(config.requirements);
  const showApiKey = requiredFields.includes("api_key");
  const showBaseUrl = requiredFields.includes("base_url");

  const form = useForm({
    onSubmit: async ({ value }) => {
      try {
        await providerMutation.mutateAsync(value);
      } catch {
        return;
      }

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

  return (
    <AccordionItem
      disabled={config.disabled || locked}
      value={config.id}
      className={cn([
        "bg-muted rounded-[22px] border-2",
        isConfigured ? "border-border border-solid" : "border-dashed",
      ])}
    >
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
          providerType === "llm" && "flex flex-col gap-6",
        ])}
      >
        {providerContext}

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
                  <ExternalLink size={12} />
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
                  <ExternalLink size={12} />
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
                  <ExternalLink size={12} />
                </a>
              )}
            </div>
          )}
          {((!showBaseUrl && config.baseUrl) || !showApiKey) && (
            <details className="flex flex-col gap-4 pt-2">
              <summary className="text-muted-foreground hover:text-foreground cursor-pointer text-xs hover:underline">
                <Trans>Advanced</Trans>
              </summary>
              <div className="mt-4 flex flex-col gap-4">
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
              </div>
            </details>
          )}
          {providerMutation.error && (
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
      components={streamdownComponents}
      className={cn(["mt-1 text-sm", className])}
      isAnimating={false}
    >
      {children}
    </Streamdown>
  );
}

function useProvider(providerType: ProviderType, id: string) {
  const providerRow = useAiProvider(providerType, id);
  const providerMutation = useSetAiProvider(providerType, id);

  const { data } = aiProviderSchema.safeParse(providerRow);
  return [data, providerMutation] as const;
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
          <Icon icon="mdi:alert-circle" size={14} />
          <span>{errorMessage}</span>
        </p>
      )}
    </div>
  );
}
