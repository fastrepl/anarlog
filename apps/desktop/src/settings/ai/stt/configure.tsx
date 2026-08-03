import { Trans } from "@lingui/react/macro";

import { Accordion } from "@anlg/ui/components/ui/accordion";

import { useSttSettings } from "./context";
import { ProviderId, PROVIDERS } from "./shared";

import { NonAnarlogProviderCard, StyledStreamdown } from "~/settings/ai/shared";
import { useConfigValue } from "~/shared/config";

export function ConfigureProviders() {
  const { accordionValue, setAccordionValue } = useSttSettings();
  const currentProvider = useConfigValue("current_stt_provider");

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
        {PROVIDERS.filter((provider) => !("builtIn" in provider)).map(
          (provider) => (
            <NonAnarlogProviderCard
              key={provider.id}
              config={provider}
              providerType="stt"
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
    providerId === "anarlog"
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
                : providerId === "openrouter"
                  ? `Use [OpenRouter](https://openrouter.ai) to transcribe with supported speech-to-text models through one API key. OpenRouter transcription runs after recording.`
                  : providerId === "cloudflare_workers_ai"
                    ? `Use a [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/) endpoint that exposes Deepgram-compatible Nova-3 transcription.`
                    : providerId === "fireworks"
                      ? `Use [Fireworks AI](https://fireworks.ai) for transcriptions.`
                      : providerId === "mistral"
                        ? `Use [Mistral](https://mistral.ai) for transcriptions.`
                        : providerId === "cohere"
                          ? `Use [Cohere Transcribe](https://docs.cohere.com/docs/transcribe) for batch transcription. Files must be 25 MB or smaller and use one selected language. Cohere does not return timestamps or speaker labels, so Anarlog estimates word timing.`
                          : providerId === "google_cloud"
                            ? `Use [Google Cloud Speech-to-Text](https://cloud.google.com/speech-to-text) synchronous recognition for recordings up to one minute and 10 MB. Paste an OAuth access token in the API key field; refresh it when it expires.`
                            : providerId === "azure_speech"
                              ? `Use [Azure AI Speech](https://learn.microsoft.com/azure/ai-services/speech-service/rest-speech-to-text) fast transcription. Enter the regional Speech resource endpoint as the Base URL and its subscription key as the API key.`
                              : providerId === "aws_transcribe"
                                ? `Amazon Transcribe's native file API requires SigV4 plus an S3 object. Enter an OpenAI-compatible gateway URL that performs that AWS authentication and upload, then paste the gateway token as the API key.`
                                : providerId === "speechmatics"
                                  ? `Use [Speechmatics](https://docs.speechmatics.com/speech-to-text/batch/quickstart) enhanced batch transcription. The default endpoint uses the EU region and can be changed under Advanced.`
                                  : providerId === "revai"
                                    ? `Use [Rev AI](https://docs.rev.ai/api/asynchronous/get-started) asynchronous transcription. Anarlog uploads the recording, waits for the job, and retrieves word timestamps and speaker labels.`
                                    : providerId === "custom"
                                      ? `We only support **Deepgram compatible** endpoints for now.`
                                      : "";

  if (!content.trim()) {
    return null;
  }

  return <StyledStreamdown className="mb-3">{content.trim()}</StyledStreamdown>;
}
