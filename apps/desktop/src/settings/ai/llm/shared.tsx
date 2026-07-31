import { Icon } from "@iconify-icon/react";
import {
  Anthropic,
  Apple,
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
import { checkAppleFoundationModelAvailability } from "~/settings/ai/shared/list-apple-foundation";
import {
  checkLMStudioAvailability,
  checkOllamaAvailability,
} from "~/settings/ai/shared/local-provider-availability";
import { sortProviders } from "~/settings/ai/shared/sort-providers";

export type Provider = {
  id: string;
  displayName: string;
  badge: string | null;
  icon: ReactNode;
  baseUrl?: string;
  requirements: ProviderRequirement[];
  checkAvailability?: (baseUrl: string, apiKey: string) => Promise<boolean>;
  hideAdvanced?: boolean;
  links?: {
    download?: { label: string; url: string };
    models?: { label: string; url: string };
    setup?: { label: string; url: string };
  };
};

const _PROVIDERS = [
  {
    id: "anarlog",
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
    id: "apple_foundation",
    displayName: "Apple Intelligence",
    badge: "Experimental",
    // The Apple mark fills its viewBox edge to edge, so it needs an explicit size to
    // escape the icon slot's `size-full` stretch and match the padded brand logos.
    icon: <Apple className="!size-4" />,
    baseUrl: undefined,
    requirements: [],
    checkAvailability: checkAppleFoundationModelAvailability,
    hideAdvanced: true,
  },
  {
    id: "lmstudio",
    displayName: "LM Studio",
    badge: null,
    icon: <LmStudio size={16} />,
    baseUrl: "http://127.0.0.1:1234/v1",
    requirements: [],
    checkAvailability: checkLMStudioAvailability,
    links: {
      download: {
        label: "Download LM Studio",
        url: "https://lmstudio.ai/download",
      },
      models: { label: "Available models", url: "https://lmstudio.ai/models" },
      setup: {
        label: "Setup guide",
        url: "https://docs.anarlog.so/ai-setup#lm-studio",
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
    checkAvailability: checkOllamaAvailability,
    links: {
      download: {
        label: "Download Ollama",
        url: "https://ollama.com/download",
      },
      models: { label: "Available models", url: "https://ollama.com/library" },
      setup: {
        label: "Setup guide",
        url: "https://docs.anarlog.so/ai-setup#ollama",
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
    id: "cohere",
    displayName: "Cohere",
    badge: null,
    icon: <Icon icon="simple-icons:cohere" width={16} />,
    baseUrl: "https://api.cohere.ai/compatibility/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.cohere.com/docs/models",
      },
      setup: {
        label: "OpenAI compatibility",
        url: "https://docs.cohere.com/docs/compatibility-api",
      },
    },
  },
  {
    id: "groq",
    displayName: "Groq",
    badge: null,
    icon: <Icon icon="simple-icons:groq" width={16} />,
    baseUrl: "https://api.groq.com/openai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://console.groq.com/docs/models",
      },
      setup: {
        label: "API keys",
        url: "https://console.groq.com/keys",
      },
    },
  },
  {
    id: "xai",
    displayName: "xAI",
    badge: null,
    icon: <Icon icon="bxl:xai" width={16} />,
    baseUrl: "https://api.x.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.x.ai/developers/models",
      },
      setup: {
        label: "API keys",
        url: "https://console.x.ai/",
      },
    },
  },
  {
    id: "together",
    displayName: "Together AI",
    badge: null,
    icon: <Icon icon="simple-icons:together" width={16} />,
    baseUrl: "https://api.together.xyz/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.together.ai/docs/serverless-models",
      },
      setup: {
        label: "API keys",
        url: "https://api.together.ai/settings/api-keys",
      },
    },
  },
  {
    id: "fireworks",
    displayName: "Fireworks AI",
    badge: null,
    icon: <Icon icon="simple-icons:fireworks" width={16} />,
    baseUrl: "https://api.fireworks.ai/inference/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://fireworks.ai/models",
      },
      setup: {
        label: "API keys",
        url: "https://fireworks.ai/account/api-keys",
      },
    },
  },
  {
    id: "cerebras",
    displayName: "Cerebras",
    badge: null,
    icon: <Icon icon="simple-icons:cerebras" width={16} />,
    baseUrl: "https://api.cerebras.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://inference-docs.cerebras.ai/models/overview",
      },
      setup: {
        label: "API keys",
        url: "https://cloud.cerebras.ai/",
      },
    },
  },
  {
    id: "amazon_bedrock",
    displayName: "Amazon Bedrock",
    badge: "Beta",
    icon: <Icon icon="simple-icons:amazonwebservices" width={16} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Supported models",
        url: "https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html",
      },
      setup: {
        label: "OpenAI-compatible APIs",
        url: "https://docs.aws.amazon.com/bedrock/latest/userguide/bedrock-mantle.html",
      },
    },
  },
  {
    id: "google_vertex_ai",
    displayName: "Google Vertex AI",
    badge: "Beta",
    icon: <Icon icon="simple-icons:googlecloud" width={16} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://cloud.google.com/vertex-ai/generative-ai/docs/learn/models",
      },
      setup: {
        label: "OpenAI compatibility",
        url: "https://cloud.google.com/vertex-ai/generative-ai/docs/multimodal/call-vertex-using-openai-library",
      },
    },
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
