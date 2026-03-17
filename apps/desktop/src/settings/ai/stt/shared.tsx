import { Icon } from "@iconify-icon/react";
import {
  AssemblyAI,
  ElevenLabs,
  Fireworks,
  Mistral,
  OpenAI,
} from "@lobehub/icons";
import type { ReactNode } from "react";

import type { AmModel, LocalModel } from "@hypr/plugin-local-stt";

import { env } from "~/env";
import {
  type ProviderRequirement,
  requiresEntitlement,
} from "~/settings/ai/shared/eligibility";
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
};

export const displayModelId = (model: string) => {
  if (model === "cloud") {
    return "Pro (Cloud)";
  }

  if (model === "stt-v4" || model === "stt-rt-v4" || model === "stt-async-v4") {
    return "Soniox v4";
  }

  if (model === "stt-v3" || model === "stt-rt-v3" || model === "stt-async-v3") {
    return "Soniox v3";
  }

  if (model === "universal") {
    return "Universal";
  }

  if (model === "solaria-1") {
    return "Solaria 1";
  }

  if (model === "scribe_v2") {
    return "Scribe V2";
  }

  if (model === "whisper-1") {
    return "Whisper 1";
  }

  if (model === "gpt-4o-transcribe") {
    return "GPT-4o Transcribe";
  }

  if (model === "gpt-4o-mini-transcribe") {
    return "GPT-4o mini Transcribe";
  }

  if (model === "voxtral-mini-2602") {
    return "Voxtral Mini Transcribe 2";
  }

  if (model.startsWith("am-")) {
    const am = model as AmModel;
    if (am == "am-parakeet-v2") {
      return "Parakeet V2";
    }
    if (am == "am-parakeet-v3") {
      return "Parakeet V3";
    }
    if (am == "am-whisper-large-v3") {
      return "Whisper Large V3";
    }
  }

  return model;
};

const _PROVIDERS = [
  {
    disabled: false,
    id: "hyprnote",
    displayName: "Char",
    badge: "Recommended",
    icon: <img src="/assets/icon.png" alt="Char" className="size-5" />,
    baseUrl: new URL("/stt", env.VITE_API_URL).toString(),
    models: [
      "cloud",
      "am-parakeet-v2",
      "am-parakeet-v3",
      "am-whisper-large-v3",
    ],
    requirements: [],
  },
  {
    disabled: false,
    id: "deepgram",
    displayName: "Deepgram",
    badge: null,
    icon: <Icon icon="simple-icons:deepgram" className="size-4" />,
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
    badge: "Beta",
    icon: <AssemblyAI size={12} />,
    baseUrl: "https://api.assemblyai.com",
    models: ["universal"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "openai",
    displayName: "OpenAI",
    badge: "Beta",
    icon: <OpenAI size={16} />,
    baseUrl: "https://api.openai.com/v1",
    models: ["gpt-4o-transcribe", "gpt-4o-mini-transcribe", "whisper-1"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "gladia",
    displayName: "Gladia",
    badge: "Beta",
    icon: (
      <img
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
      <img
        src="/assets/soniox.jpeg"
        alt="Soniox"
        className="size-5 rounded-xs"
      />
    ),
    baseUrl: "https://api.soniox.com",
    models: ["stt-v4", "stt-v3"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "elevenlabs",
    displayName: "ElevenLabs",
    badge: "Beta",
    icon: <ElevenLabs size={16} />,
    baseUrl: "https://api.elevenlabs.io",
    models: ["scribe_v2"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "mistral",
    displayName: "Mistral",
    badge: "Beta",
    icon: <Mistral size={16} />,
    baseUrl: "https://api.mistral.ai/v1",
    models: ["voxtral-mini-2602"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    disabled: false,
    id: "custom",
    displayName: "Custom",
    badge: null,
    icon: <Icon icon="mingcute:random-fill" />,
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
    icon: <Fireworks size={16} />,
    baseUrl: "https://api.fireworks.ai",
    models: ["Default"],
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
] as const satisfies readonly Provider[];

export const PROVIDERS = sortProviders(_PROVIDERS);
export type ProviderId = (typeof _PROVIDERS)[number]["id"];

export const sttProviderRequiresPro = (providerId: ProviderId) => {
  const provider = PROVIDERS.find((p) => p.id === providerId);
  return provider ? requiresEntitlement(provider.requirements, "pro") : false;
};
