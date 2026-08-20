import { Trans } from "@lingui/react/macro";
import { useMemo, useState } from "react";

import { Accordion } from "@anlg/ui/components/ui/accordion";

import { useLlmSettings } from "./context";
import { ProviderId, PROVIDERS } from "./shared";
import {
  isSubscriptionProviderId,
  type SubscriptionProviderId,
} from "./subscriptions";
import { ConnectProvidersMenu } from "./subscriptions/connect";

import {
  filterProviders,
  NonAnarlogProviderCard,
  ProviderSearch,
  providerRowId,
  StyledStreamdown,
} from "~/settings/ai/shared";
import { useAiProvidersState } from "~/settings/providers";
import { useConfigValue } from "~/shared/config";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useLlmSettings();
  const currentProvider = useConfigValue("current_llm_provider");
  const { providers: configuredProviders } = useAiProvidersState("llm");
  const [search, setSearch] = useState("");
  const [connectingId, setConnectingId] =
    useState<SubscriptionProviderId | null>(null);
  const configuredIds = useMemo(() => {
    const ids = new Set<string>();
    for (const provider of PROVIDERS) {
      if (provider.id === "anarlog") {
        continue;
      }
      const config = configuredProviders[providerRowId("llm", provider.id)];
      if (config?.api_key.trim()) {
        ids.add(provider.id);
      }
    }
    return ids;
  }, [configuredProviders]);
  const providers = filterProviders(
    PROVIDERS.filter((provider) => {
      if (provider.id === "anarlog") {
        return false;
      }
      if (
        provider.authKind === "subscription" &&
        !configuredIds.has(provider.id) &&
        !search.trim()
      ) {
        return false;
      }
      return true;
    }),
    search,
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        <h3 className="text-md font-sans font-semibold">
          <Trans>Configure Providers</Trans>
        </h3>
        <ProviderSearch value={search} onChange={setSearch} />
      </div>
      <Accordion
        type="single"
        collapsible
        className="flex flex-col gap-3"
        value={accordionValue}
        onValueChange={setAccordionValue}
      >
        {providers.map((provider) => {
          const providerId = provider.id;
          return (
            <NonAnarlogProviderCard
              key={provider.id}
              config={provider}
              providerType="llm"
              providers={PROVIDERS}
              providerContext={
                <ProviderContext providerId={provider.id as ProviderId} />
              }
              currentProvider={currentProvider}
              onConnect={
                isSubscriptionProviderId(providerId)
                  ? () => setConnectingId(providerId)
                  : undefined
              }
            />
          );
        })}
      </Accordion>
      {providers.length === 0 && search.trim() ? (
        <p className="text-muted-foreground py-8 text-center text-sm">
          <Trans>No providers found.</Trans>
        </p>
      ) : null}
      <ConnectProvidersMenu
        providers={PROVIDERS}
        configuredIds={configuredIds}
        connectingId={connectingId}
        onConnectingIdChange={setConnectingId}
        onSelectApiProvider={setAccordionValue}
      />
    </div>
  );
}

function ProviderContext({ providerId }: { providerId: ProviderId }) {
  const content =
    providerId === "claude"
      ? "Uses your **Claude Pro or Max** plan. Sign in through Connect — no Anthropic API key needed."
      : providerId === "chatgpt"
        ? "Uses your **ChatGPT Plus or Pro** plan. After signing in, paste the redirect URL from your browser."
        : providerId === "grok"
          ? "Uses your **SuperGrok or X Premium+** plan through xAI's subscription login."
          : providerId === "github_copilot"
            ? "Uses your **GitHub Copilot** plan. Approve the device code in the browser to connect."
            : providerId === "kimi_code"
              ? "Uses your **Kimi Code** membership. Paste the coding API key from the Kimi Code console."
              : providerId === "apple_foundation"
                ? "- Uses Apple's on-device **System Language Model**.\n- Requires macOS 26 or later, a Mac that supports Apple Intelligence, and Apple Intelligence turned on.\n- This experiment is text-only and works best with shorter transcripts."
                : providerId === "lmstudio"
                  ? "- Ensure LM Studio server is **running.** (Default port is 1234)\n- Enable **CORS** in LM Studio config."
                  : providerId === "ollama"
                    ? "- Ensure Ollama is **running** (`ollama serve`)\n- Pull a model first (`ollama pull llama3.2`)"
                    : providerId === "unsloth"
                      ? "- Ensure the Unsloth server is **running.** (Default port is 8888)\n- Paste the API key from Unsloth. It starts with `sk-unsloth-`.\n- Only models **loaded** in Unsloth show up in the list."
                      : providerId === "custom"
                        ? "We only support **OpenAI-compatible** endpoints for now."
                        : providerId === "openrouter"
                          ? "We filter out models from the combobox based on heuristics like **input modalities** and **tool support**."
                          : providerId === "moonshot"
                            ? "Uses Moonshot AI's **OpenAI-compatible Kimi API**. The default endpoint is the international service and can be changed under Advanced."
                            : providerId === "zai"
                              ? "Uses Z.AI's **OpenAI-compatible GLM API**. The default endpoint is the international service and can be changed under Advanced."
                              : providerId === "alibaba_cloud"
                                ? "Uses Alibaba Cloud Model Studio's **OpenAI-compatible API**. The default endpoint is the Singapore region; change the Base URL under Advanced when your API key belongs to another region."
                                : providerId === "siliconflow"
                                  ? "Uses SiliconFlow's **OpenAI-compatible API**. The default endpoint is the international service; use `https://api.siliconflowcn/v1` under Advanced for a China-region API key."
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
                                            : providerId ===
                                                "cloudflare_workers_ai"
                                              ? "Enter the Workers AI **OpenAI-compatible base URL** as `https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1` and use a Cloudflare API token with Workers AI access."
                                              : "";

  if (!content) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content}</StyledStreamdown>;
}
