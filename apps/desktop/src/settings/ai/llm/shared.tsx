import { Icon } from "@iconify-icon/react";
import {
  Anthropic,
  Azure,
  AzureAI,
  LmStudio,
  Mistral,
  Ollama,
  OpenAI,
  OpenRouter,
} from "@lobehub/icons";
import type { ReactNode } from "react";

import { env } from "~/env";
import { AnarlogProviderIcon } from "~/settings/ai/shared";
import { type ProviderRequirement } from "~/settings/ai/shared/eligibility";
import { sortProviders } from "~/settings/ai/shared/sort-providers";

type Provider = {
  id: string;
  displayName: string;
  badge: string | null;
  icon: ReactNode;
  baseUrl?: string;
  requirements: ProviderRequirement[];
  links?: {
    download?: { label: string; url: string };
    models?: { label: string; url: string };
    setup?: { label: string; url: string };
  };
};

const _PROVIDERS = [
  {
    id: "hyprnote",
    displayName: "Anarlog",
    badge: "Recommended",
    icon: <AnarlogProviderIcon />,
    baseUrl: new URL("/llm", env.VITE_API_URL).toString(),
    requirements: [
      { kind: "requires_auth" },
      { kind: "requires_entitlement", entitlement: "pro" },
    ],
  },
  {
    id: "lmstudio",
    displayName: "LM Studio",
    badge: null,
    icon: <LmStudio size={16} />,
    baseUrl: "http://127.0.0.1:1234/v1",
    requirements: [],
    links: {
      download: {
        label: "Download LM Studio",
        url: "https://lmstudio.ai/download",
      },
      models: { label: "Available models", url: "https://lmstudio.ai/models" },
      setup: {
        label: "Setup guide",
        url: "https://char.com/docs/faq/local-llm-setup/#lm-studio-setup",
      },
    },
  },
  {
    id: "ollama",
    displayName: "Ollama",
    badge: null,
    icon: <Ollama size={16} />,
    baseUrl: "http://127.0.0.1:11434/v1",
    requirements: [],
    links: {
      download: {
        label: "Download Ollama",
        url: "https://ollama.com/download",
      },
      models: { label: "Available models", url: "https://ollama.com/library" },
      setup: {
        label: "Setup guide",
        url: "https://char.com/docs/faq/local-llm-setup/#ollama-setup",
      },
    },
  },
  {
    id: "openrouter",
    displayName: "OpenRouter",
    badge: null,
    icon: <OpenRouter size={16} />,
    baseUrl: "https://openrouter.ai/api/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    id: "openai",
    displayName: "OpenAI",
    badge: null,
    icon: <OpenAI size={16} />,
    baseUrl: "https://api.openai.com/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    id: "cloudflare_workers_ai",
    displayName: "Cloudflare Workers AI",
    badge: null,
    icon: <Icon icon="simple-icons:cloudflare" width={16} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://developers.cloudflare.com/workers-ai/models/",
      },
      setup: {
        label: "Setup guide",
        url: "https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/",
      },
    },
  },
  {
    id: "anthropic",
    displayName: "Anthropic",
    badge: null,
    icon: <Anthropic size={16} />,
    baseUrl: "https://api.anthropic.com/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    id: "mistral",
    displayName: "Mistral",
    badge: null,
    icon: <Mistral size={16} />,
    baseUrl: "https://api.mistral.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    id: "azure_openai",
    displayName: "Azure OpenAI",
    badge: "Beta",
    icon: <Azure size={14} style={{ height: 14, width: 14 }} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
  },
  {
    id: "azure_ai",
    displayName: "Azure AI Foundry",
    badge: "Beta",
    icon: <AzureAI size={14} style={{ height: 14, width: 14 }} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
  },
  {
    id: "google_generative_ai",
    displayName: "Google Gemini",
    badge: null,
    icon: <Icon icon="simple-icons:googlegemini" width={16} />,
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
  },
  {
    id: "custom",
    displayName: "Custom",
    badge: null,
    icon: <Icon icon="mingcute:random-fill" />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
  },
] as const satisfies readonly Provider[];

export const PROVIDERS = sortProviders(_PROVIDERS);
export type ProviderId = (typeof _PROVIDERS)[number]["id"];
