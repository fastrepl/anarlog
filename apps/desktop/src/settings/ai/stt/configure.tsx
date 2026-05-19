import { Accordion } from "@hypr/ui/components/ui/accordion";

import { useSttSettings } from "./context";
import { ProviderId, PROVIDERS } from "./shared";

import { NonHyprProviderCard, StyledStreamdown } from "~/settings/ai/shared";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useSttSettings();

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
        {PROVIDERS.filter((provider) => provider.id !== "hyprnote").map(
          (provider) => (
            <NonHyprProviderCard
              key={provider.id}
              config={provider}
              providerType="stt"
              providers={PROVIDERS}
              providerContext={<ProviderContext providerId={provider.id} />}
            />
          ),
        )}
      </Accordion>
    </div>
  );
}

function ProviderContext({ providerId }: { providerId: ProviderId }) {
  const content =
    providerId === "hyprnote"
      ? "**Anarlog Cloud** routes request to the **best available model** for highest accuracy and performance."
      : providerId === "deepgram"
        ? `Use [Deepgram](https://deepgram.com) for transcriptions. \
    If you want to use a [Dedicated](https://developers.deepgram.com/reference/custom-endpoints#deepgram-dedicated-endpoints)
    or [EU](https://developers.deepgram.com/reference/custom-endpoints#eu-endpoints) endpoint,
    you can do that in the **advanced** section.`
        : providerId === "soniox"
          ? `Use [Soniox](https://soniox.com) for transcriptions.`
          : providerId === "assemblyai"
            ? `Use [AssemblyAI](https://www.assemblyai.com) for transcriptions.`
            : providerId === "gladia"
              ? `Use [Gladia](https://www.gladia.io) for transcriptions.`
              : providerId === "openai"
                ? `Use [OpenAI](https://openai.com) for transcriptions.`
                : providerId === "fireworks"
                  ? `Use [Fireworks AI](https://fireworks.ai) for transcriptions.`
                  : providerId === "mistral"
                    ? `Use [Mistral](https://mistral.ai) for transcriptions.`
                    : providerId === "custom"
                      ? `We only support **Deepgram compatible** endpoints for now.`
                      : "";

  if (!content.trim()) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content.trim()}</StyledStreamdown>;
}
