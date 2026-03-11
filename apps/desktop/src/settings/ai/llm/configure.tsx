import { useCallback } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

import { useLlmSettings } from "./context";
import { ProviderId, PROVIDERS } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import {
  HyprCloudCTAButton,
  HyprProviderRow,
  NonHyprProviderCard,
  StyledStreamdown,
} from "~/settings/ai/shared";
import * as settings from "~/store/tinybase/store/settings";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useLlmSettings();

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-md font-serif font-semibold">Configure Providers</h3>
      <Accordion
        type="single"
        collapsible
        className="flex flex-col gap-3"
        value={accordionValue}
        onValueChange={setAccordionValue}
      >
        <HyprProviderCard
          providerId="hyprnote"
          providerName="Char"
          icon={<img src="/assets/icon.png" alt="Char" className="size-5" />}
          badge={PROVIDERS.find((p) => p.id === "hyprnote")?.badge}
        />
        {PROVIDERS.filter((provider) => provider.id !== "hyprnote").map(
          (provider) => (
            <NonHyprProviderCard
              key={provider.id}
              config={provider}
              providerType="llm"
              providers={PROVIDERS}
              providerContext={<ProviderContext providerId={provider.id} />}
            />
          ),
        )}
      </Accordion>
    </div>
  );
}

function HyprProviderCard({
  providerId,
  providerName,
  icon,
  badge,
}: {
  providerId: ProviderId;
  providerName: string;
  icon: React.ReactNode;
  badge?: string | null;
}) {
  const { hyprAccordionRef, shouldHighlight } = useLlmSettings();
  const auth = useAuth();
  const isConfigured = !!auth?.session;

  return (
    <AccordionItem
      ref={hyprAccordionRef}
      value={providerId}
      className={cn([
        "rounded-xl border-2 bg-neutral-50",
        isConfigured ? "border-solid border-neutral-300" : "border-dashed",
      ])}
    >
      <AccordionTrigger className="gap-2 px-4 capitalize hover:no-underline">
        <div className="flex items-center gap-2">
          {icon}
          <span>{providerName}</span>
          {badge && (
            <span className="rounded-full border border-neutral-300 px-2 text-xs font-light text-neutral-500">
              {badge}
            </span>
          )}
        </div>
      </AccordionTrigger>
      <AccordionContent className="px-4">
        <ProviderContext providerId={providerId} />
        <div className="flex flex-col gap-3">
          <HyprProviderAutoRow highlight={shouldHighlight} />
        </div>
      </AccordionContent>
    </AccordionItem>
  );
}

function HyprProviderAutoRow({ highlight }: { highlight?: boolean }) {
  const { isPro, canStartTrial, upgradeToPro } = useBillingAccess();

  const handleSelectProvider = settings.UI.useSetValueCallback(
    "current_llm_provider",
    (provider: string) => provider,
    [],
    settings.STORE_ID,
  );

  const handleSelectModel = settings.UI.useSetValueCallback(
    "current_llm_model",
    (model: string) => model,
    [],
    settings.STORE_ID,
  );

  const handleClick = useCallback(() => {
    if (!isPro) {
      upgradeToPro();
    } else {
      handleSelectProvider("hyprnote");
      handleSelectModel("Auto");
    }
  }, [isPro, upgradeToPro, handleSelectProvider, handleSelectModel]);

  return (
    <HyprProviderRow>
      <div className="flex-1">
        <span className="text-sm font-medium">Char Cloud</span>
        <p className="text-xs text-neutral-500">
          Use the Char Cloud API for AI assistance.
        </p>
      </div>
      <HyprCloudCTAButton
        isPro={isPro}
        canStartTrial={canStartTrial.data}
        highlight={highlight}
        onClick={handleClick}
      />
    </HyprProviderRow>
  );
}

function ProviderContext({ providerId }: { providerId: ProviderId }) {
  const content =
    providerId === "hyprnote"
      ? "A curated set of models we continuously test to provide the **best performance & reliability**."
      : providerId === "lmstudio"
        ? "- Ensure LM Studio server is **running.** (Default port is 1234)\n- Enable **CORS** in LM Studio config."
        : providerId === "ollama"
          ? "- Ensure Ollama is **running** (`ollama serve`)\n- Pull a model first (`ollama pull llama3.2`)"
          : providerId === "custom"
            ? "We only support **OpenAI-compatible** endpoints for now."
            : providerId === "openrouter"
              ? "We filter out models from the combobox based on heuristics like **input modalities** and **tool support**."
              : providerId === "azure_openai"
                ? "Enter your **Azure OpenAI endpoint** (e.g. `https://your-resource.openai.azure.com`) as the Base URL and your **API key**. [Report issues](https://github.com/fastrepl/char/issues/3928)"
                : providerId === "azure_ai"
                  ? "Enter your **Azure AI Foundry endpoint** as the Base URL and your **API key**. Supports Claude and other models deployed via Azure AI Foundry. [Report issues](https://github.com/fastrepl/char/issues/3928)"
                  : providerId === "google_generative_ai"
                    ? "Visit [AI Studio](https://aistudio.google.com/api-keys) to create an API key."
                    : "";

  if (!content) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content}</StyledStreamdown>;
}
