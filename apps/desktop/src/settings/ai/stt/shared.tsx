import { Icon } from "@iconify-icon/react";
import {
  AssemblyAI,
  Cloudflare,
  ElevenLabs,
  Fireworks,
  Mistral,
  OpenAI,
} from "@lobehub/icons";
import type { ReactNode } from "react";

import type { LocalModel } from "@hypr/plugin-local-stt";

import { env } from "~/env";
import { AnarlogProviderIcon, ProviderBrandImage } from "~/settings/ai/shared";
import { type ProviderRequirement } from "~/settings/ai/shared/eligibility";
import { sortProviders } from "~/settings/ai/shared/sort-providers";
import { localSttQueries } from "~/stt/useLocalSttModel";

export { localSttQueries as sttModelQueries };

type Provider = {
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

export const displayModelId = (model: string) => {
  if (model === "cloud") {
    return "Pro (Cloud)";
  }

  if (model === "nova-3" || model === "nova-3-general") {
    return "Nova 3";
  }

  if (model === "nova-3-medical") {
    return "Nova 3 Medical";
  }

  if (model === "u3-rt-pro") {
    return "Universal-3 Pro Streaming";
  }

  if (model === "universal-3-pro" || model === "universal") {
    return "Universal-3 Pro";
  }

  if (model === "whisper-rt") {
    return "Whisper RT";
  }

  if (model === "stt-v5" || model === "stt-rt-v5" || model === "stt-async-v5") {
    return "Soniox v5";
  }

  if (model === "stt-v4" || model === "stt-rt-v4" || model === "stt-async-v4") {
    return "Soniox v4";
  }

  if (model === "stt-v3" || model === "stt-rt-v3" || model === "stt-async-v3") {
    return "Soniox v3";
  }

  if (model === "solaria-1") {
    return "Solaria 1";
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

  if (model === "ink-whisper") {
    return "Ink Whisper";
  }

  if (model === "ink-2") {
    return "Ink 2";
  }

  if (model === "gpt-4o-transcribe") {
    return "GPT-4o Transcribe";
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

  if (model === "parakeet-tdt-0.6b-v3") {
    return "Parakeet TDT 0.6B V3";
  }

  if (model === "faster-whisper-large-v3-turbo") {
    return "Faster Whisper Large V3 Turbo";
  }

  return model;
};

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
    id: "hyprnote",
    displayName: "Anarlog",
    badge: "Recommended",
    icon: <AnarlogProviderIcon />,
    baseUrl: new URL("/stt", env.VITE_API_URL).toString(),
    models: ["cloud"],
    requirements: [],
  },
  {
    disabled: false,
    id: "deepgram",
    displayName: "Deepgram",
    badge: null,
    icon: (
      <Icon icon="simple-icons:deepgram" className="text-foreground size-4" />
    ),
    baseUrl: "https://api.deepgram.com/v1",
    models: [
      "nova-3-general",
      "nova-3-medical",
      "nova-2-general",
      "nova-2-meeting",
      "nova-2-phonecall",
      "nova-2-finance",
      "nova-2-conversationalai",
      "nova-2-voicemail",
      "nova-2-video",
      "nova-2-medical",
      "nova-2-drivethru",
      "nova-2-automotive",
      "nova-2-atc",
    ],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "assemblyai",
    displayName: "AssemblyAI",
    badge: null,
    icon: <AssemblyAI size={16} style={{ height: 16, width: 16 }} />,
    baseUrl: "https://api.assemblyai.com",
    models: ["universal-3-pro", "u3-rt-pro"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "openai",
    displayName: "OpenAI",
    badge: "Batch only",
    icon: <OpenAI size={14} />,
    baseUrl: "https://api.openai.com/v1",
    models: ["gpt-4o-transcribe", "gpt-4o-mini-transcribe", "whisper-1"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "cartesia",
    displayName: "Cartesia",
    badge: null,
    icon: <Icon icon="mingcute:voiceprint-fill" className="size-4" />,
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
    icon: <Cloudflare size={14} />,
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
    icon: (
      <ProviderBrandImage
        src="/assets/gladia.jpeg"
        alt="Gladia"
        className="size-4 rounded-xs"
      />
    ),
    baseUrl: "https://api.gladia.io",
    models: ["solaria-1"],
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
        className="size-5 rounded-xs"
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
    icon: <ElevenLabs size={14} style={{ height: 14, width: 14 }} />,
    baseUrl: "https://api.elevenlabs.io",
    models: ["scribe_v2", "scribe_v2_realtime"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "mistral",
    displayName: "Mistral",
    badge: null,
    icon: <Mistral size={14} />,
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
        className="size-5"
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
        className="size-3.5 rounded-xs"
      />
    ),
    baseUrl: "https://api.aquavoice.com/api/v1",
    models: ["avalon-v1-en"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "custom",
    displayName: "Custom",
    badge: null,
    icon: (
      <Icon icon="mingcute:random-fill" className="text-foreground size-4" />
    ),
    baseUrl: undefined,
    models: [],
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
  },
  {
    disabled: true,
    id: "fireworks",
    displayName: "Fireworks",
    badge: null,
    icon: <Fireworks size={14} />,
    baseUrl: "https://api.fireworks.ai",
    models: ["Default"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
] as const satisfies readonly Provider[];

export const PROVIDERS = sortProviders(_PROVIDERS);
export type ProviderId = (typeof _PROVIDERS)[number]["id"];
