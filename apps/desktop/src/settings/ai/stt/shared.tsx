import {
  AlibabaCloud,
  AssemblyAI,
  Apple,
  Aws,
  Azure,
  Cloudflare,
  Cohere,
  ElevenLabs,
  Fireworks,
  Gemini,
  GoogleCloud,
  Groq,
  Mistral,
  OpenAI,
  OpenRouter,
  SiliconCloud,
  Together,
  XAI,
  ZAI,
} from "@lobehub/icons";
import { FolderOpen, Shuffle, Waveform } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import type { LocalModel } from "@anlg/plugin-local-stt";

import { env } from "~/env";
import {
  AnarlogProviderIcon,
  ProviderBrandImage,
  ProviderLobeIcon,
} from "~/settings/ai/shared";
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

  if (model === "local-file") {
    return "Local model file";
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

  if (model === "universal-3-5-pro-realtime") {
    return "Universal 3.5 Pro Realtime";
  }

  if (model === "universal-3-5-pro") {
    return "Universal 3.5 Pro";
  }

  if (model === "u3-rt-pro") {
    return "Universal 3 Pro Realtime";
  }

  if (model === "universal-3-pro" || model === "universal") {
    return "Universal 3 Pro";
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

  if (model === "qwen3-asr-flash-realtime") {
    return "Qwen3 ASR Flash Realtime";
  }

  if (model === "glm-asr-2512") {
    return "GLM ASR";
  }

  if (model === "FunAudioLLM/SenseVoiceSmall") {
    return "SenseVoice Small";
  }

  if (model === "TeleAI/TeleSpeechASR") {
    return "TeleSpeech ASR";
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

  if (
    model === "gemini-3.5-transcribe-live" ||
    model === "gemini-3.5-transcribe-live-preview"
  ) {
    return "3.5 Transcribe Live";
  }

  if (
    model === "gemini-3.5-transcribe" ||
    model === "gemini-3.5-transcribe-preview"
  ) {
    return "3.5 Transcribe";
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

const DEPRECATED_STT_MODELS: Record<string, readonly string[]> = {
  assemblyai: ["universal-3-pro", "u3-rt-pro"],
  openai: ["gpt-4o-transcribe", "gpt-4o-mini-transcribe"],
  openrouter: ["openai/gpt-4o-transcribe", "openai/gpt-4o-mini-transcribe"],
  soniox: ["stt-rt-v4", "stt-async-v4", "stt-v4"],
};

export function isDeprecatedSttModel(
  provider?: string | null,
  model?: string | null,
) {
  if (!provider || !model) {
    return false;
  }

  return DEPRECATED_STT_MODELS[provider]?.includes(model) === true;
}

export function displayModelLabel(model: string, displayName?: string) {
  return displayName ?? displayModelId(model);
}

export function formatDownloadProgress(progress?: number | null) {
  if (progress == null || progress <= 0) {
    return null;
  }

  return `${Math.round(progress)}%`;
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

const styles = stylex.create({
  roundedBrand: {
    borderRadius: "0.125rem",
  },
});

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
    icon: <ProviderLobeIcon icon={Apple} />,
    models: [],
    requirements: [],
  },
  {
    disabled: false,
    id: "local_file",
    displayName: "Local file",
    badge: "Batch only",
    baseUrl: "",
    builtIn: true,
    icon: <FolderOpen />,
    models: ["local-file"],
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
    links: {
      models: {
        label: "Available models",
        url: "https://developers.deepgram.com/docs/models-languages-overview",
      },
      setup: {
        label: "API setup",
        url: "https://console.deepgram.com/",
      },
    },
  },
  {
    disabled: false,
    id: "assemblyai",
    displayName: "AssemblyAI",
    badge: null,
    icon: <ProviderLobeIcon icon={AssemblyAI} />,
    baseUrl: "https://api.assemblyai.com",
    models: [
      "universal-3-5-pro",
      "universal-3-5-pro-realtime",
      "universal-3-pro",
      "u3-rt-pro",
    ],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://www.assemblyai.com/docs/speech-to-text",
      },
      setup: {
        label: "API setup",
        url: "https://www.assemblyai.com/dashboard",
      },
    },
  },
  {
    disabled: false,
    id: "openai",
    displayName: "OpenAI",
    badge: null,
    icon: <ProviderLobeIcon icon={OpenAI} />,
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
    links: {
      models: {
        label: "Available models",
        url: "https://platform.openai.com/docs/guides/speech-to-text",
      },
      setup: {
        label: "API setup",
        url: "https://platform.openai.com/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "openrouter",
    displayName: "OpenRouter",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={OpenRouter} />,
    baseUrl: "https://openrouter.ai/api/v1",
    models: [
      "openai/gpt-transcribe",
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
        label: "Available models",
        url: "https://openrouter.ai/models?output_modalities=transcription",
      },
      setup: {
        label: "API setup",
        url: "https://openrouter.ai/settings/keys",
      },
    },
  },
  {
    disabled: false,
    id: "dashscope",
    displayName: "Alibaba Cloud Model Studio",
    badge: null,
    icon: <ProviderLobeIcon icon={AlibabaCloud} />,
    baseUrl: "https://dashscope-intl.aliyuncs.com",
    models: ["qwen3-asr-flash-realtime"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://www.alibabacloud.com/help/en/model-studio/real-time-speech-recognition-user-guide",
      },
      setup: {
        label: "API setup",
        url: "https://www.alibabacloud.com/help/en/model-studio/get-api-key",
      },
    },
  },
  {
    disabled: false,
    id: "zai",
    displayName: "Z.AI",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={ZAI} />,
    baseUrl: "https://api.z.ai/api/paas/v4",
    models: ["glm-asr-2512"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.z.ai/guides/audio/glm-asr-2512",
      },
      setup: {
        label: "API setup",
        url: "https://docs.z.ai/api-reference/introduction",
      },
    },
  },
  {
    disabled: false,
    id: "siliconflow",
    displayName: "SiliconFlow",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={SiliconCloud} />,
    baseUrl: "https://api.siliconflow.com/v1",
    models: ["FunAudioLLM/SenseVoiceSmall", "TeleAI/TeleSpeechASR"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.siliconflow.com/en/api-reference/audio/create-audio-transcriptions",
      },
      setup: {
        label: "API setup",
        url: "https://docs.siliconflow.com/en/userguide/quickstart",
      },
    },
  },
  {
    disabled: false,
    id: "groq",
    displayName: "Groq",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={Groq} />,
    baseUrl: "https://api.groq.com/openai/v1",
    models: ["whisper-large-v3-turbo", "whisper-large-v3"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://console.groq.com/docs/speech-to-text",
      },
      setup: {
        label: "API setup",
        url: "https://console.groq.com/keys",
      },
    },
  },
  {
    disabled: false,
    id: "xai",
    displayName: "xAI",
    badge: null,
    icon: <ProviderLobeIcon icon={XAI} />,
    baseUrl: "https://api.x.ai/v1",
    models: ["xai-stt"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.x.ai/developers/model-capabilities/audio/speech-to-text",
      },
      setup: {
        label: "API setup",
        url: "https://console.x.ai/",
      },
    },
  },
  {
    disabled: false,
    id: "together",
    displayName: "Together AI",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={Together} />,
    baseUrl: "https://api.together.xyz/v1",
    models: ["openai/whisper-large-v3"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.together.ai/docs/inference-transcription",
      },
      setup: {
        label: "API setup",
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
        label: "Available models",
        url: "https://docs.speechmatics.com/speech-to-text/batch/quickstart",
      },
      setup: {
        label: "API setup",
        url: "https://portal.speechmatics.com/settings/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "azure_speech",
    displayName: "Azure AI Speech",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={Azure} />,
    baseUrl: undefined,
    models: ["fast-transcription"],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://learn.microsoft.com/azure/ai-services/speech-service/fast-transcription-create",
      },
      setup: {
        label: "API setup",
        url: "https://learn.microsoft.com/azure/ai-services/speech-service/get-started-speech-to-text",
      },
    },
  },
  {
    disabled: false,
    id: "google_cloud",
    displayName: "Google Cloud Speech-to-Text",
    badge: "Short batch",
    icon: <ProviderLobeIcon icon={GoogleCloud} />,
    baseUrl: "https://speech.googleapis.com/v1",
    models: ["latest_long"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://cloud.google.com/speech-to-text/docs/transcription-model",
      },
      setup: {
        label: "API setup",
        url: "https://cloud.google.com/speech-to-text/docs/authentication",
      },
    },
  },
  {
    disabled: false,
    id: "google_generative_ai",
    displayName: "Google Gemini",
    badge: null,
    icon: <ProviderLobeIcon icon={Gemini} />,
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    models: ["gemini-3.5-transcribe-live", "gemini-3.5-transcribe"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://ai.google.dev/gemini-api/docs/models",
      },
      setup: {
        label: "API setup",
        url: "https://aistudio.google.com/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "aws_transcribe",
    displayName: "Amazon Transcribe",
    badge: "Gateway",
    icon: <ProviderLobeIcon icon={Aws} />,
    baseUrl: undefined,
    models: ["amazon-transcribe"],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.aws.amazon.com/transcribe/latest/dg/how-it-works.html",
      },
      setup: {
        label: "API setup",
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
        label: "Available models",
        url: "https://docs.rev.ai/api/asynchronous/get-started",
      },
      setup: {
        label: "API setup",
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
        label: "Available models",
        url: "https://docs.cartesia.ai/api-reference/stt/transcribe",
      },
      setup: {
        label: "API setup",
        url: "https://play.cartesia.ai/keys",
      },
    },
  },
  {
    disabled: false,
    id: "cloudflare_workers_ai",
    displayName: "Cloudflare Workers AI",
    badge: null,
    icon: <ProviderLobeIcon icon={Cloudflare} />,
    baseUrl: undefined,
    models: ["nova-3"],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://developers.cloudflare.com/workers-ai/models/nova-3/",
      },
      setup: {
        label: "API setup",
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
    links: {
      models: {
        label: "Available models",
        url: "https://docs.gladia.io/",
      },
      setup: {
        label: "API setup",
        url: "https://app.gladia.io/",
      },
    },
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
        sx={styles.roundedBrand}
      />
    ),
    baseUrl: "https://api.soniox.com",
    models: ["stt-rt-v5", "stt-rt-v4"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://soniox.com/docs/stt/models",
      },
      setup: {
        label: "API setup",
        url: "https://console.soniox.com/",
      },
    },
  },
  {
    disabled: false,
    id: "elevenlabs",
    displayName: "ElevenLabs",
    badge: null,
    icon: <ProviderLobeIcon icon={ElevenLabs} />,
    baseUrl: "https://api.elevenlabs.io",
    models: ["scribe_v2", "scribe_v2_realtime"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://elevenlabs.io/docs/capabilities/speech-to-text",
      },
      setup: {
        label: "API setup",
        url: "https://elevenlabs.io/app/settings/api-keys",
      },
    },
  },
  {
    disabled: false,
    id: "mistral",
    displayName: "Mistral",
    badge: null,
    icon: <ProviderLobeIcon icon={Mistral} />,
    baseUrl: "https://api.mistral.ai/v1",
    models: ["voxtral-mini-2602", "voxtral-mini-transcribe-realtime-2602"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.mistral.ai/capabilities/audio/",
      },
      setup: {
        label: "API setup",
        url: "https://console.mistral.ai/api-keys",
      },
    },
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
    links: {
      models: {
        label: "Available models",
        url: "https://docs.pyannote.ai/",
      },
      setup: {
        label: "API setup",
        url: "https://dashboard.pyannote.ai/",
      },
    },
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
        sx={styles.roundedBrand}
      />
    ),
    baseUrl: "https://api.aquavoice.com/api/v1",
    models: ["avalon-v1-en"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://aquavoice.com/avalon-api/docs",
      },
      setup: {
        label: "API setup",
        url: "https://app.aquavoice.com/api-dashboard",
      },
    },
  },
  {
    disabled: false,
    id: "cohere",
    displayName: "Cohere",
    badge: "Batch only",
    icon: <ProviderLobeIcon icon={Cohere} />,
    baseUrl: "https://api.cohere.com/v2",
    models: ["cohere-transcribe-03-2026"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.cohere.com/docs/transcribe",
      },
      setup: {
        label: "API setup",
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
    icon: <ProviderLobeIcon icon={Fireworks} />,
    baseUrl: "https://api.fireworks.ai",
    models: ["whisper-v3-turbo"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.fireworks.ai/guides/querying-asr-models",
      },
      setup: {
        label: "API setup",
        url: "https://fireworks.ai/account/api-keys",
      },
    },
  },
] as const satisfies readonly Provider[];

const PROVIDER_ORDER = [
  "soniqo",
  "apple_speech",
  "local_file",
  "deepgram",
  "assemblyai",
  "openai",
  "openrouter",
  "dashscope",
  "zai",
  "siliconflow",
  "google_generative_ai",
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
