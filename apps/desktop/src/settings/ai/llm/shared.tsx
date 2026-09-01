import {
  AlibabaCloud,
  Anthropic,
  Apple,
  Aws,
  Azure,
  AzureAI,
  Cerebras,
  Cloudflare,
  Cohere,
  DeepSeek,
  Fireworks,
  Gemini,
  GithubCopilot,
  GoogleCloud,
  Groq,
  LmStudio,
  Mistral,
  Moonshot,
  Ollama,
  OpenAI,
  OpenRouter,
  SiliconCloud,
  Together,
  XAI,
  ZAI,
} from "@lobehub/icons";
import { Shuffle } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import { env } from "~/env";
import { AnarlogProviderIcon, ProviderLobeIcon } from "~/settings/ai/shared";
import { type ProviderRequirement } from "~/settings/ai/shared/eligibility";
import { checkAppleFoundationModelAvailability } from "~/settings/ai/shared/list-apple-foundation";
import {
  checkLMStudioAvailability,
  checkOllamaAvailability,
  checkUnslothAvailability,
} from "~/settings/ai/shared/local-provider-availability";
import { sortProviders } from "~/settings/ai/shared/sort-providers";

export type Provider = {
  id: string;
  displayName: string;
  badge: string | null;
  icon: ReactNode;
  baseUrl?: string;
  authKind?: "api" | "subscription";
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
    id: "chatgpt",
    displayName: "ChatGPT",
    badge: "Subscription",
    icon: <ProviderLobeIcon icon={OpenAI} />,
    baseUrl: "https://chatgpt.com/backend-api/codex",
    authKind: "subscription",
    hideAdvanced: true,
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      setup: {
        label: "ChatGPT plans",
        url: "https://chatgpt.com/",
      },
    },
  },
  {
    id: "grok",
    displayName: "Grok",
    badge: "Subscription",
    icon: <ProviderLobeIcon icon={XAI} />,
    baseUrl: "https://api.x.ai/v1",
    authKind: "subscription",
    hideAdvanced: true,
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      setup: {
        label: "SuperGrok",
        url: "https://grok.com/",
      },
    },
  },
  {
    id: "github_copilot",
    displayName: "GitHub Copilot",
    badge: "Subscription",
    icon: <ProviderLobeIcon icon={GithubCopilot} />,
    baseUrl: "https://api.githubcopilot.com",
    authKind: "subscription",
    hideAdvanced: true,
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      setup: {
        label: "GitHub Copilot",
        url: "https://github.com/features/copilot",
      },
    },
  },
  {
    id: "kimi_code",
    displayName: "Kimi Code",
    badge: "Subscription",
    icon: <ProviderLobeIcon icon={Moonshot} />,
    baseUrl: "https://api.kimi.com/coding/v1",
    authKind: "subscription",
    hideAdvanced: true,
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      setup: {
        label: "Kimi Code membership",
        url: "https://www.kimi.com/en/help/kimi-code/membership-guide",
      },
    },
  },
  {
    id: "apple_foundation",
    displayName: "Apple Intelligence",
    badge: "Experimental",
    icon: <ProviderLobeIcon icon={Apple} />,
    baseUrl: undefined,
    requirements: [],
    checkAvailability: checkAppleFoundationModelAvailability,
    hideAdvanced: true,
  },
  {
    id: "lmstudio",
    displayName: "LM Studio",
    badge: null,
    icon: <ProviderLobeIcon icon={LmStudio} />,
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
    icon: <ProviderLobeIcon icon={Ollama} />,
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
    id: "unsloth",
    displayName: "Unsloth",
    badge: null,
    // Rendered unfiltered: the brand filter flattens this multi-color mark
    // into a solid blob in dark mode.
    icon: (
      <img
        src="/assets/unsloth-mark.png"
        alt="Unsloth"
        className="size-full object-contain object-center"
      />
    ),
    baseUrl: "http://127.0.0.1:8888/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    checkAvailability: checkUnslothAvailability,
    links: {
      download: {
        label: "Download Unsloth",
        url: "https://unsloth.ai/docs/desktop",
      },
      models: {
        label: "Available models",
        url: "https://huggingface.co/unsloth",
      },
      setup: {
        label: "Setup guide",
        url: "https://docs.anarlog.so/ai-setup#unsloth",
      },
    },
  },
  {
    id: "openrouter",
    displayName: "OpenRouter",
    badge: null,
    icon: <ProviderLobeIcon icon={OpenRouter} />,
    baseUrl: "https://openrouter.ai/api/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://openrouter.ai/models",
      },
      setup: {
        label: "API setup",
        url: "https://openrouter.ai/settings/keys",
      },
    },
  },
  {
    id: "openai",
    displayName: "OpenAI",
    badge: null,
    icon: <ProviderLobeIcon icon={OpenAI} />,
    baseUrl: "https://api.openai.com/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://platform.openai.com/docs/models",
      },
      setup: {
        label: "API setup",
        url: "https://platform.openai.com/api-keys",
      },
    },
  },
  {
    id: "moonshot",
    displayName: "Moonshot AI",
    badge: null,
    icon: <ProviderLobeIcon icon={Moonshot} />,
    baseUrl: "https://api.moonshot.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://platform.kimi.ai/docs/api/list-models",
      },
      setup: {
        label: "API setup",
        url: "https://platform.kimi.ai/docs/overview",
      },
    },
  },
  {
    id: "zai",
    displayName: "Z.AI",
    badge: null,
    icon: <ProviderLobeIcon icon={ZAI} />,
    baseUrl: "https://api.z.ai/api/paas/v4",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.z.ai/guides/overview/overview",
      },
      setup: {
        label: "API setup",
        url: "https://docs.z.ai/api-reference/introduction",
      },
    },
  },
  {
    id: "deepseek",
    displayName: "DeepSeek",
    badge: null,
    icon: <ProviderLobeIcon icon={DeepSeek} />,
    baseUrl: "https://api.deepseek.com",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://api-docs.deepseek.com/quick_start/pricing",
      },
      setup: {
        label: "API setup",
        url: "https://platform.deepseek.com/api_keys",
      },
    },
  },
  {
    id: "alibaba_cloud",
    displayName: "Alibaba Cloud Model Studio",
    badge: null,
    icon: <ProviderLobeIcon icon={AlibabaCloud} />,
    baseUrl: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://www.alibabacloud.com/help/en/model-studio/getting-started/models",
      },
      setup: {
        label: "API setup",
        url: "https://www.alibabacloud.com/help/en/model-studio/base-url",
      },
    },
  },
  {
    id: "siliconflow",
    displayName: "SiliconFlow",
    badge: null,
    icon: <ProviderLobeIcon icon={SiliconCloud} />,
    baseUrl: "https://api.siliconflow.com/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.siliconflow.com/en/userguide/introduction",
      },
      setup: {
        label: "API setup",
        url: "https://docs.siliconflow.com/en/userguide/quickstart",
      },
    },
  },
  {
    id: "cohere",
    displayName: "Cohere",
    badge: null,
    icon: <ProviderLobeIcon icon={Cohere} />,
    baseUrl: "https://api.cohere.ai/compatibility/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.cohere.com/docs/models",
      },
      setup: {
        label: "API setup",
        url: "https://docs.cohere.com/docs/compatibility-api",
      },
    },
  },
  {
    id: "groq",
    displayName: "Groq",
    badge: null,
    icon: <ProviderLobeIcon icon={Groq} />,
    baseUrl: "https://api.groq.com/openai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://console.groq.com/docs/models",
      },
      setup: {
        label: "API setup",
        url: "https://console.groq.com/keys",
      },
    },
  },
  {
    id: "xai",
    displayName: "xAI",
    badge: null,
    icon: <ProviderLobeIcon icon={XAI} />,
    baseUrl: "https://api.x.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.x.ai/developers/models",
      },
      setup: {
        label: "API setup",
        url: "https://console.x.ai/",
      },
    },
  },
  {
    id: "together",
    displayName: "Together AI",
    badge: null,
    icon: <ProviderLobeIcon icon={Together} />,
    baseUrl: "https://api.together.xyz/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.together.ai/docs/serverless-models",
      },
      setup: {
        label: "API setup",
        url: "https://api.together.ai/settings/api-keys",
      },
    },
  },
  {
    id: "fireworks",
    displayName: "Fireworks AI",
    badge: null,
    icon: <ProviderLobeIcon icon={Fireworks} />,
    baseUrl: "https://api.fireworks.ai/inference/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://fireworks.ai/models",
      },
      setup: {
        label: "API setup",
        url: "https://fireworks.ai/account/api-keys",
      },
    },
  },
  {
    id: "cerebras",
    displayName: "Cerebras",
    badge: null,
    icon: <ProviderLobeIcon icon={Cerebras} />,
    baseUrl: "https://api.cerebras.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://inference-docs.cerebras.ai/models/overview",
      },
      setup: {
        label: "API setup",
        url: "https://cloud.cerebras.ai/",
      },
    },
  },
  {
    id: "amazon_bedrock",
    displayName: "Amazon Bedrock",
    badge: "Beta",
    icon: <ProviderLobeIcon icon={Aws} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html",
      },
      setup: {
        label: "API setup",
        url: "https://docs.aws.amazon.com/bedrock/latest/userguide/bedrock-mantle.html",
      },
    },
  },
  {
    id: "google_vertex_ai",
    displayName: "Google Vertex AI",
    badge: "Beta",
    icon: <ProviderLobeIcon icon={GoogleCloud} />,
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
        label: "API setup",
        url: "https://cloud.google.com/vertex-ai/generative-ai/docs/multimodal/call-vertex-using-openai-library",
      },
    },
  },
  {
    id: "cloudflare_workers_ai",
    displayName: "Cloudflare Workers AI",
    badge: null,
    icon: <ProviderLobeIcon icon={Cloudflare} />,
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
        label: "API setup",
        url: "https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/",
      },
    },
  },
  {
    id: "anthropic",
    displayName: "Anthropic",
    badge: null,
    icon: <ProviderLobeIcon icon={Anthropic} />,
    baseUrl: "https://api.anthropic.com/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.anthropic.com/en/docs/about-claude/models",
      },
      setup: {
        label: "API setup",
        url: "https://console.anthropic.com/settings/keys",
      },
    },
  },
  {
    id: "mistral",
    displayName: "Mistral",
    badge: null,
    icon: <ProviderLobeIcon icon={Mistral} />,
    baseUrl: "https://api.mistral.ai/v1",
    requirements: [{ kind: "requires_config", fields: ["api_key"] }],
    links: {
      models: {
        label: "Available models",
        url: "https://docs.mistral.ai/getting-started/models/",
      },
      setup: {
        label: "API setup",
        url: "https://console.mistral.ai/api-keys",
      },
    },
  },
  {
    id: "azure_openai",
    displayName: "Azure OpenAI",
    badge: "Beta",
    icon: <ProviderLobeIcon icon={Azure} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://learn.microsoft.com/azure/ai-foundry/openai/concepts/models",
      },
      setup: {
        label: "API setup",
        url: "https://learn.microsoft.com/azure/ai-foundry/openai/how-to/create-resource",
      },
    },
  },
  {
    id: "azure_ai",
    displayName: "Azure AI Foundry",
    badge: "Beta",
    icon: <ProviderLobeIcon icon={AzureAI} />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
    links: {
      models: {
        label: "Available models",
        url: "https://learn.microsoft.com/azure/ai-foundry/how-to/model-catalog-overview",
      },
      setup: {
        label: "API setup",
        url: "https://ai.azure.com/",
      },
    },
  },
  {
    id: "google_generative_ai",
    displayName: "Google Gemini",
    badge: null,
    icon: <ProviderLobeIcon icon={Gemini} />,
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
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
    id: "custom",
    displayName: "Custom",
    badge: null,
    icon: <Shuffle weight="fill" />,
    baseUrl: undefined,
    requirements: [
      { kind: "requires_config", fields: ["base_url", "api_key"] },
    ],
  },
] as const satisfies readonly Provider[];

const PROVIDER_ORDER = [
  "chatgpt",
  "grok",
  "github_copilot",
  "kimi_code",
  "openai",
  "anthropic",
  "google_generative_ai",
  "openrouter",
  "moonshot",
  "zai",
  "deepseek",
  "alibaba_cloud",
  "siliconflow",
  "amazon_bedrock",
  "azure_openai",
  "google_vertex_ai",
  "azure_ai",
  "groq",
  "ollama",
  "xai",
  "mistral",
  "together",
  "cohere",
  "fireworks",
  "cloudflare_workers_ai",
  "cerebras",
  "lmstudio",
  "unsloth",
  "apple_foundation",
] as const;

export const PROVIDERS: Provider[] = sortProviders(_PROVIDERS, PROVIDER_ORDER);
export type ProviderId = (typeof _PROVIDERS)[number]["id"];
