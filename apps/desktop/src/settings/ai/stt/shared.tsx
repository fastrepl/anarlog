import {
  AssemblyAI,
  Apple,
  Aws,
  Azure,
  Cloudflare,
  Cohere,
  ElevenLabs,
  Fireworks,
  GoogleCloud,
  Groq,
  Mistral,
  OpenAI,
  OpenRouter,
  Together,
  XAI,
} from "@lobehub/icons";
import { Shuffle, Waveform } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import type { LocalModel } from "@anlg/plugin-local-stt";

import { env } from "~/env";
import { AnarlogProviderIcon, ProviderBrandImage } from "~/settings/ai/shared";
import { type ProviderRequirement } from "~/settings/ai/shared/eligibility";
import { sortProviders } from "~/settings/ai/shared/sort-providers";
import { localSttQueries } from "~/stt/useLocalSttModel";

export { localSttQueries as sttModelQueries };

type Provider = {
  builtIn?: boolean;
  disabled: boolean;
  id: string;
  displayName: string;
  icon: ReactNode;
  baseUrl?: string;
  models: LocalModel[] | string[];
  badge?: string | null;
  requirements: ProviderRequirement[];
  links?: {
    models?: { label: string; url: string };
    setup?: { label: string; url: string };
  };
};

const OPENROUTER_MODEL_LABELS: Record<string, string> = {
  "fish-audio/transcribe-1": "Transcribe 1",
  "x-ai/grok-stt-1.0": "Grok STT 1.0",
  "deepgram/nova-3": "Nova 3",
  "microsoft/mai-transcribe-1.5": "MAI Transcribe 1.5",
  "nvidia/parakeet-tdt-0.6b-v3": "Parakeet TDT 0.6B V3",
  "mistralai/voxtral-mini-transcribe": "Voxtral Mini Transcribe",
  "qwen/qwen3-asr-flash-2026-02-10": "Qwen3 ASR Flash",
  "google/chirp-3": "Chirp 3",
};

export const displayModelId = (model: string): string => {
  if (model === "cloud") {
    return "Pro (Cloud)";
  }

  if (model === "nova-3" || model === "nova-3-general") {
    return "Nova 3";
  }

  if (model === "nova-3-medical") {
    return "Nova 3 Medical";
  }

  if (model === "flux-general-multi") {
    return "Flux General Multilingual";
  }

  if (model === "flux-general-en") {
    return "Flux General English";
  }

  if (model === "u3-rt-pro") {
    return "Universal 3.5 Pro Realtime";
  }

  if (model === "universal-3-pro" || model === "universal") {
    return "Universal 3.5 Pro";
  }

  if (model === "whisper-rt") {
    return "Whisper RT";
  }

  if (model === "stt-v5" || model === "stt-rt-v5" || model === "stt-async-v5") {
    return "Soniox 5";
  }

  if (model === "stt-v4" || model === "stt-rt-v4" || model === "stt-async-v4") {
    return "Soniox 4";
  }

  if (model === "stt-v3" || model === "stt-rt-v3" || model === "stt-async-v3") {
    return "Soniox 3";
  }

  if (model === "solaria-1") {
    return "Solaria 1";
  }

  if (model === "solaria-3") {
    return "Solaria 3";
  }

  if (model === "scribe_v2_realtime") {
    return "Scribe V2 Realtime";
  }

  if (model === "scribe_v2") {
    return "Scribe V2";
  }

  if (model === "whisper-1") {
    return "Whisper 1";
  }

  if (model === "gpt-live-transcribe") {
    return "GPT Live Transcribe";
  }

  if (model === "gpt-transcribe") {
    return "GPT Transcribe";
  }

  if (model === "ink-whisper") {
    return "Ink Whisper";
  }

  if (model === "ink-2") {
    return "Ink 2";
  }

  if (model === "gpt-4o-transcribe") {
    return "GPT-4o Transcribe";
  }

  if (model === "gpt-4o-transcribe-diarize") {
    return "GPT-4o Transcribe Diarize";
  }

  if (model === "gpt-4o-mini-transcribe") {
    return "GPT-4o mini Transcribe";
  }

  if (model === "voxtral-mini-transcribe-realtime-2602") {
    return "Voxtral Realtime";
  }

  if (model === "voxtral-mini-2602") {
    return "Voxtral Mini Transcribe 2";
  }

  if (model === "avalon-v1-en") {
    return "Avalon V1";
  }

  if (model === "cohere-transcribe-03-2026") {
    return "Cohere Transcribe";
  }

  if (model === "whisper-large-v3-turbo") {
    return "Whisper Large V3 Turbo";
  }

  if (model === "whisper-large-v3") {
    return "Whisper Large V3";
  }

  if (model === "openai/whisper-large-v3") {
    return "Whisper Large V3";
  }

  if (model === "xai-stt") {
    return "xAI Speech to Text";
  }

  if (model === "enhanced") {
    return "Enhanced";
  }

  if (model === "fast-transcription") {
    return "Fast Transcription";
  }

  if (model === "latest_long") {
    return "Latest Long";
  }

  if (model === "amazon-transcribe") {
    return "Amazon Transcribe";
  }

  if (model === "machine") {
    return "Machine Transcription";
  }

  if (model === "apple-speech") {
    return "Apple Speech";
  }

  if (model === "soniqo-parakeet-streaming") {
    return "Parakeet Streaming";
  }

  if (model === "soniqo-parakeet-batch") {
    return "Parakeet Batch";
  }

  if (model === "soniqo-omnilingual") {
    return "Omnilingual ASR";
  }

  if (model === "soniqo-qwen3-small") {
    return "Qwen3 ASR 0.6B";
  }

  if (model === "soniqo-qwen3-large") {
    return "Qwen3 ASR 1.7B";
  }

  if (model === "parakeet-tdt-0.6b-v3") {
    return "Parakeet TDT 0.6B V3";
  }

  if (model === "faster-whisper-large-v3-turbo") {
    return "Faster Whisper Large V3 Turbo";
  }

  const openRouterLabel = OPENROUTER_MODEL_LABELS[model];
  if (openRouterLabel) {
    return openRouterLabel;
  }

  if (model.startsWith("openai/")) {
    return displayModelId(model.slice("openai/".length));
  }

  return model;
};

export function displayModelLabel(model: string, displayName?: string) {
  return displayName ?? displayModelId(model);
}

export function formatModelSize(sizeBytes?: number | null) {
  if (!sizeBytes) {
    return null;
  }

  const unit = sizeBytes >= 1024 * 1024 * 1024 ? "GB" : "MB";
  const value =
    unit === "GB" ? sizeBytes / 1024 / 1024 / 1024 : sizeBytes / 1024 / 1024;

  return `~${value.toLocaleString(undefined, {
    maximumFractionDigits: value >= 10 ? 0 : 1,
  })} ${unit}`;
}

const _PROVIDERS = [
  {
    disabled: false,
    id: "anarlog",
    displayName: "Anarlog",
    badge: "Recommended",
    builtIn: true,
    icon: <AnarlogProviderIcon />,
    baseUrl: new URL("/stt", env.VITE_API_URL).toString(),
    models: ["cloud"],
    requirements: [],
  },
  {
    disabled: false,
    id: "soniqo",
    displayName: "Soniqo",
    badge: "On device",
    baseUrl: "",
    builtIn: true,
    icon: <Waveform />,
    models: [],
    requirements: [],
  },
  {
    disabled: false,
    id: "apple_speech",
    displayName: "Apple Speech",
    badge: "On device",
    baseUrl: "",
    builtIn: true,
    icon: <Apple />,
    models: [],
    requirements: [],
  },
  {
    disabled: false,
    id: "deepgram",
    displayName: "Deepgram",
    badge: null,
    icon: <ProviderBrandImage src="/assets/deepgram-mark.svg" alt="Deepgram" />,
    baseUrl: "https://api.deepgram.com/v1",
    models: [
      "flux-general-multi",
      "flux-general-en",
      "nova-3-general",
      "nova-3-medical",
    ],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "assemblyai",
    displayName: "AssemblyAI",
    badge: null,
    icon: <AssemblyAI />,
    baseUrl: "https://api.assemblyai.com",
    models: ["universal-3-pro", "u3-rt-pro"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "openai",
    displayName: "OpenAI",
    badge: null,
    icon: <OpenAI />,
    baseUrl: "https://api.openai.com/v1",
    models: [
      "gpt-live-transcribe",
      "gpt-transcribe",
      "gpt-4o-transcribe-diarize",
      "gpt-4o-transcribe",
      "gpt-4o-mini-transcribe",
      "whisper-1",
    ],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "openrouter",
    displayName: "OpenRouter",
    badge: "Batch only",
    icon: <OpenRouter />,
    baseUrl: "https://openrouter.ai/api/v1",
    models: [
      "openai/gpt-4o-mini-transcribe",
      "openai/gpt-4o-transcribe",
      "mistralai/voxtral-mini-transcribe",
      "openai/whisper-large-v3-turbo",
      "openai/whisper-large-v3",
      "fish-audio/transcribe-1",
      "x-ai/grok-stt-1.0",
      "deepgram/nova-3",
      "microsoft/mai-transcribe-1.5",
      "nvidia/parakeet-tdt-0.6b-v3",
      "qwen/qwen3-asr-flash-2026-02-10",
      "google/chirp-3",
      "openai/whisper-1",
    ],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Transcription models",
        url: "https://openrouter.ai/models?output_modalities=transcription",
      },
      setup: {
        label: "API keys",
        url: "https://openrouter.ai/settings/keys",
      },
    },
  },
  {
    disabled: false,
    id: "groq",
    displayName: "Groq",
    badge: "Batch only",
    icon: <Groq />,
    baseUrl: "https://api.groq.com/openai/v1",
    models: ["whisper-large-v3-turbo", "whisper-large-v3"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Speech-to-text models",
        url: "https://console.groq.com/docs/speech-to-text",
      },
      setup: {
        label: "API keys",
        url: "https://console.groq.com/keys",
      },
    },
  },
  {
    disabled: false,
    id: "xai",
    displayName: "xAI",
    badge: null,
    icon: <XAI />,
    baseUrl: "https://api.x.ai/v1",
    models: ["xai-stt"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Speech-to-text docs",
        url: "https://docs.x.ai/developers/model-capabilities/audio/speech-to-text",
      },
      setup: {
        label: "API keys",
        url: "https://console.x.ai/",
      },
    },
  },
  {
    disabled: false,
    id: "together",
    displayName: "Together AI",
    badge: "Batch only",
    icon: <Together />,
    baseUrl: "https://api.together.xyz/v1",
    models: ["openai/whisper-large-v3"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Transcription docs",
        url: "https://docs.together.ai/docs/inference-transcription",
      },
      setup: {
        label: "API keys",
        url: "https://api.together.ai/settings/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "speechmatics",
    displayName: "Speechmatics",
    badge: "Batch only",
    icon: (
      <ProviderBrandImage
        src="/assets/speechmatics-mark.svg"
        alt="Speechmatics"
      />
    ),
    baseUrl: "https://eu1.asr.api.speechmatics.com/v2",
    models: ["enhanced"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Batch transcription docs",
        url: "https://docs.speechmatics.com/speech-to-text/batch/quickstart",
      },
      setup: {
        label: "API keys",
        url: "https://portal.speechmatics.com/settings/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "azure_speech",
    displayName: "Azure AI Speech",
    badge: "Batch only",
    icon: <Azure />,
    baseUrl: undefined,
    models: ["fast-transcription"],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Fast transcription docs",
        url: "https://learn.microsoft.com/azure/ai-services/speech-service/fast-transcription-create",
      },
      setup: {
        label: "Speech resource setup",
        url: "https://learn.microsoft.com/azure/ai-services/speech-service/get-started-speech-to-text",
      },
    },
  },
  {
    disabled: false,
    id: "google_cloud",
    displayName: "Google Cloud Speech-to-Text",
    badge: "Short batch",
    icon: <GoogleCloud />,
    baseUrl: "https://speech.googleapis.com/v1",
    models: ["latest_long"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Speech-to-Text models",
        url: "https://cloud.google.com/speech-to-text/docs/transcription-model",
      },
      setup: {
        label: "Authentication",
        url: "https://cloud.google.com/speech-to-text/docs/authentication",
      },
    },
  },
  {
    disabled: false,
    id: "aws_transcribe",
    displayName: "Amazon Transcribe",
    badge: "Gateway",
    icon: <Aws />,
    baseUrl: undefined,
    models: ["amazon-transcribe"],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Amazon Transcribe docs",
        url: "https://docs.aws.amazon.com/transcribe/latest/dg/how-it-works.html",
      },
      setup: {
        label: "Authentication requirements",
        url: "https://docs.aws.amazon.com/transcribe/latest/dg/getting-started-http-websocket.html",
      },
    },
  },
  {
    disabled: false,
    id: "revai",
    displayName: "Rev AI",
    badge: "Batch only",
    icon: <ProviderBrandImage src="/assets/revai-mark.svg" alt="Rev AI" />,
    baseUrl: "https://api.rev.ai/speechtotext/v1",
    models: ["machine"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Asynchronous transcription docs",
        url: "https://docs.rev.ai/api/asynchronous/get-started",
      },
      setup: {
        label: "Access tokens",
        url: "https://www.rev.ai/access_token",
      },
    },
  },
  {
    disabled: false,
    id: "cartesia",
    displayName: "Cartesia",
    badge: null,
    icon: <ProviderBrandImage src="/assets/cartesia-mark.svg" alt="Cartesia" />,
    baseUrl: "https://api.cartesia.ai",
    models: ["ink-2"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Cartesia STT docs",
        url: "https://docs.cartesia.ai/api-reference/stt/transcribe",
      },
      setup: {
        label: "API keys",
        url: "https://play.cartesia.ai/keys",
      },
    },
  },
  {
    disabled: false,
    id: "cloudflare_workers_ai",
    displayName: "Cloudflare Workers AI",
    badge: null,
    icon: <Cloudflare />,
    baseUrl: undefined,
    models: ["nova-3"],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Nova-3 docs",
        url: "https://developers.cloudflare.com/workers-ai/models/nova-3/",
      },
      setup: {
        label: "Workers AI docs",
        url: "https://developers.cloudflare.com/workers-ai/",
      },
    },
  },
  {
    disabled: false,
    id: "gladia",
    displayName: "Gladia",
    badge: null,
    icon: <ProviderBrandImage src="/assets/gladia-mark.svg" alt="Gladia" />,
    baseUrl: "https://api.gladia.io",
    models: ["solaria-3", "solaria-1"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "soniox",
    displayName: "Soniox",
    badge: null,
    icon: (
      <ProviderBrandImage
        src="/assets/soniox-black.png"
        alt="Soniox"
        className="rounded-xs"
      />
    ),
    baseUrl: "https://api.soniox.com",
    models: ["stt-rt-v5", "stt-rt-v4"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "elevenlabs",
    displayName: "ElevenLabs",
    badge: null,
    icon: <ElevenLabs />,
    baseUrl: "https://api.elevenlabs.io",
    models: ["scribe_v2", "scribe_v2_realtime"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "mistral",
    displayName: "Mistral",
    badge: null,
    icon: <Mistral />,
    baseUrl: "https://api.mistral.ai/v1",
    models: ["voxtral-mini-2602", "voxtral-mini-transcribe-realtime-2602"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "pyannote",
    displayName: "pyannoteAI",
    badge: "Batch only",
    icon: (
      <ProviderBrandImage
        src="/assets/pyannote-logo-black.png"
        alt="pyannoteAI"
      />
    ),
    baseUrl: "https://api.pyannote.ai",
    models: ["parakeet-tdt-0.6b-v3", "faster-whisper-large-v3-turbo"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "aquavoice",
    displayName: "AquaVoice",
    badge: "Batch only",
    icon: (
      <ProviderBrandImage
        src="/assets/aquavoice-black.png"
        alt="AquaVoice"
        className="rounded-xs"
      />
    ),
    baseUrl: "https://api.aquavoice.com/api/v1",
    models: ["avalon-v1-en"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "cohere",
    displayName: "Cohere",
    badge: "Batch only",
    icon: <Cohere />,
    baseUrl: "https://api.cohere.com/v2",
    models: ["cohere-transcribe-03-2026"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Cohere Transcribe docs",
        url: "https://docs.cohere.com/docs/transcribe",
      },
      setup: {
        label: "API keys",
        url: "https://dashboard.cohere.com/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "custom",
    displayName: "Custom",
    badge: null,
    icon: <Shuffle weight="fill" />,
    baseUrl: undefined,
    models: [],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
  },
  {
    disabled: false,
    id: "fireworks",
    displayName: "Fireworks",
    badge: null,
    icon: <Fireworks />,
    baseUrl: "https://api.fireworks.ai",
    models: ["whisper-v3-turbo"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Audio transcription docs",
        url: "https://docs.fireworks.ai/guides/querying-asr-models",
      },
      setup: {
        label: "API keys",
        url: "https://fireworks.ai/account/api-keys",
      },
    },
  },
] as const satisfies readonly Provider[];

const PROVIDER_ORDER = [
  "soniqo",
  "apple_speech",
  "deepgram",
  "assemblyai",
  "openai",
  "openrouter",
  "google_cloud",
  "aws_transcribe",
  "azure_speech",
  "elevenlabs",
  "soniox",
  "speechmatics",
  "groq",
  "mistral",
  "revai",
  "gladia",
  "cartesia",
  "cloudflare_workers_ai",
  "together",
  "fireworks",
  "xai",
  "pyannote",
  "cohere",
  "aquavoice",
] as const;

export const PROVIDERS = sortProviders(_PROVIDERS, PROVIDER_ORDER);
export type ProviderId = (typeof _PROVIDERS)[number]["id"];
