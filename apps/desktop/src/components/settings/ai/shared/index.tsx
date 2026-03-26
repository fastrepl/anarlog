import { Icon } from "@iconify-icon/react";
import { type AnyFieldApi, useForm } from "@tanstack/react-form";
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
import { cn } from "@hypr/utils";

import { useBillingAccess } from "../../../../billing";
import * as settings from "../../../../store/tinybase/store/settings";
import {
  getProviderSelectionBlockers,
  getRequiredConfigFields,
  type ProviderRequirement,
  requiresEntitlement,
} from "./eligibility";

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
};

function useIsProviderConfigured(
  providerId: string,
  providerType: ProviderType,
  providers: readonly ProviderConfig[],
) {
  const billing = useBillingAccess();
  const query =
    providerType === "stt"
      ? settings.QUERIES.sttProviders
      : settings.QUERIES.llmProviders;

  const configuredProviders = settings.UI.useResultTable(
    query,
    settings.STORE_ID,
  );
  const providerDef = providers.find((p) => p.id === providerId);
  const config = configuredProviders[providerId];

  if (!providerDef) {
    return false;
  }

  const baseUrl = String(config?.base_url || providerDef.baseUrl || "").trim();
  const apiKey = String(config?.api_key || "").trim();

  return (
    getProviderSelectionBlockers(providerDef.requirements, {
      isAuthenticated: true,
      isPro: billing.isPro,
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
  const billing = useBillingAccess();
  const [provider, setProvider] = useProvider(config.id);
  const locked =
    requiresEntitlement(config.requirements, "pro") && !billing.isPro;
  const isConfigured = useIsProviderConfigured(
    config.id,
    providerType,
    providers,
  );

  const requiredFields = getRequiredConfigFields(config.requirements);
  const showApiKey = requiredFields.includes("api_key");
  const showBaseUrl = requiredFields.includes("base_url");

  const form = useForm({
    onSubmit: ({ value }) => {
      void analyticsCommands.event({
        event: "ai_provider_configured",
        provider: value.type,
      });
      void analyticsCommands.setProperties({
        set: {
          has_configured_ai: true,
        },
      });
      setProvider(value);
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
        "rounded-xl border-2 bg-neutral-50",
        isConfigured ? "border-solid border-neutral-300" : "border-dashed",
      ])}
    >
      <AccordionTrigger
        className={cn([
          "capitalize gap-2 px-4 hover:no-underline",
          (config.disabled || locked) && "cursor-not-allowed opacity-30",
        ])}
      >
        <div className="flex items-center gap-2">
          {config.icon}
          <span>{config.displayName}</span>
          {config.badge && (
            <span className="text-xs text-neutral-500 font-light border border-neutral-300 rounded-full px-2">
              {config.badge}
            </span>
          )}
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
              {(field) => <FormField field={field} label="Base URL" />}
            </form.Field>
          )}
          {showApiKey && (
            <form.Field name="api_key">
              {(field) => (
                <FormField
                  field={field}
                  label="API Key"
                  placeholder="Enter your API key"
                  type="password"
                />
              )}
            </form.Field>
          )}
          {((!showBaseUrl && config.baseUrl) || !showApiKey) && (
            <details className="flex flex-col gap-4 pt-2">
              <summary className="text-xs cursor-pointer text-neutral-600 hover:text-neutral-900 hover:underline">
                Advanced
              </summary>
              <div className="mt-4 flex flex-col gap-4">
                {!showBaseUrl && config.baseUrl && (
                  <form.Field name="base_url">
                    {(field) => <FormField field={field} label="Base URL" />}
                  </form.Field>
                )}
                {!showApiKey && (
                  <form.Field name="api_key">
                    {(field) => (
                      <FormField
                        field={field}
                        label="API Key"
                        placeholder="Enter your API key (optional)"
                        type="password"
                      />
                    )}
                  </form.Field>
                )}
              </div>
            </details>
          )}
        </form>
      </AccordionContent>
    </AccordionItem>
  );
}

const streamdownComponents = {
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => {
    return (
      <ul className="list-disc pl-6 mb-1 block relative">
        {props.children as React.ReactNode}
      </ul>
    );
  },
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => {
    return (
      <ol className="list-decimal pl-6 mb-1 block relative">
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
      className={cn(["text-sm mt-1", className])}
      isAnimating={false}
    >
      {children}
    </Streamdown>
  );
}

function useProvider(id: string) {
  const providerRow = settings.UI.useRow("ai_providers", id, settings.STORE_ID);
  const setProvider = settings.UI.useSetPartialRowCallback(
    "ai_providers",
    id,
    (row: Partial<AIProvider>) => row,
    [id],
    settings.STORE_ID,
  ) as (row: Partial<AIProvider>) => void;

  const { data } = aiProviderSchema.safeParse(providerRow);
  return [data, setProvider] as const;
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
      <InputGroup className="bg-white">
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
        <p className="text-destructive text-xs flex items-center gap-1.5">
          <Icon icon="mdi:alert-circle" size={14} />
          <span>{errorMessage}</span>
        </p>
      )}
    </div>
  );
}
