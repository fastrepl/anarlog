import { Trans } from "@lingui/react/macro";

import { Accordion } from "@anlg/ui/components/ui/accordion";

import { useLlmSettings } from "./context";
import { ProviderId, PROVIDERS } from "./shared";

import { NonAnarlogProviderCard, StyledStreamdown } from "~/settings/ai/shared";
import { useConfigValue } from "~/shared/config";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useLlmSettings();
  const currentProvider = useConfigValue("current_llm_provider");

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-md font-sans font-semibold">
        <Trans>Configure Providers</Trans>
      </h3>
      <Accordion
        type="single"
        collapsible
        className="flex flex-col gap-3"
        value={accordionValue}
        onValueChange={setAccordionValue}
      >
        {PROVIDERS.filter((provider) => provider.id !== "anarlog").map(
          (provider) => (
            <NonAnarlogProviderCard
              key={provider.id}
              config={provider}
              providerType="llm"
              providers={PROVIDERS}
              providerContext={<ProviderContext providerId={provider.id} />}
              currentProvider={currentProvider}
            />
          ),
        )}
      </Accordion>
    </div>
  );
}

function ProviderContext({ providerId }: { providerId: ProviderId }) {
  const content =
    providerId === "apple_foundation"
      ? "- Uses Apple's on-device **System Language Model**.\n- Requires macOS 26 or later, a Mac that supports Apple Intelligence, and Apple Intelligence turned on.\n- This experiment is text-only and works best with shorter transcripts."
      : providerId === "lmstudio"
        ? "- Ensure LM Studio server is **running.** (Default port is 1234)\n- Enable **CORS** in LM Studio config."
        : providerId === "ollama"
          ? "- Ensure Ollama is **running** (`ollama serve`)\n- Pull a model first (`ollama pull llama3.2`)"
          : providerId === "custom"
            ? "We only support **OpenAI-compatible** endpoints for now."
            : providerId === "openrouter"
              ? "We filter out models from the combobox based on heuristics like **input modalities** and **tool support**."
              : providerId === "azure_openai"
                ? "Enter your **Azure OpenAI endpoint** (e.g. `https://your-resource.openai.azure.com`) as the Base URL and your **API key**. [Report issues](https://anarlog.so/discord)"
                : providerId === "azure_ai"
                  ? "Enter your **Azure AI Foundry endpoint** as the Base URL and your **API key**. Supports Claude and other models deployed via Azure AI Foundry. [Report issues](https://anarlog.so/discord)"
                  : providerId === "google_generative_ai"
                    ? "Visit [AI Studio](https://aistudio.google.com/api-keys) to create an API key."
                    : providerId === "amazon_bedrock"
                      ? "Enter the regional **Bedrock Mantle OpenAI-compatible URL** (for example, `https://bedrock-mantle.us-east-1.api.aws/v1`) and a Bedrock long-term API key."
                      : providerId === "google_vertex_ai"
                        ? "Enter your project and location's **Vertex AI OpenAI-compatible endpoint** and a bearer access token. Vertex access tokens expire, so replace the saved token when Google Cloud refreshes it."
                        : providerId === "cloudflare_workers_ai"
                          ? "Enter the Workers AI **OpenAI-compatible base URL** as `https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1` and use a Cloudflare API token with Workers AI access."
                          : "";

  if (!content) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content}</StyledStreamdown>;
}
