import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { getLlmProviderStatus } from "./select";
import { PROVIDERS } from "./shared";

function provider(id: string) {
  const provider = PROVIDERS.find((p) => p.id === id);
  if (!provider) {
    throw new Error(`Provider not found: ${id}`);
  }
  return provider;
}

describe("LLM providers", () => {
  test("orders providers by popularity", () => {
    expect(PROVIDERS.map(({ id }) => id)).toEqual([
      "anarlog",
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
      "custom",
    ]);
  });

  test("bundles every provider icon", () => {
    for (const { icon } of PROVIDERS) {
      const markup = renderToStaticMarkup(icon);

      expect(markup).toMatch(/<(img|svg)\b/);
      expect(markup).not.toContain("iconify-icon");
    }
  });
});

describe("getLlmProviderStatus", () => {
  test("does not configure API-key providers without a saved key", () => {
    const status = getLlmProviderStatus({
      provider: provider("openai"),
      config: { api_key: "" },
      isAuthenticated: false,
      isPaid: false,
    });

    expect(status.configured).toBe(false);
    expect(status.listModels).toBeUndefined();
  });

  test("configures API-key providers when a key is saved", () => {
    const status = getLlmProviderStatus({
      provider: provider("openai"),
      config: { api_key: "sk-test" },
      isAuthenticated: false,
      isPaid: false,
    });

    expect(status.configured).toBe(true);
    expect(status.listModels).toBeTypeOf("function");
  });

  test.each(["chatgpt", "grok", "github_copilot", "kimi_code"])(
    "treats %s as a subscription provider that needs a saved credential",
    (id) => {
      const definition = provider(id);
      const missing = getLlmProviderStatus({
        provider: definition,
        config: { api_key: "" },
        isAuthenticated: false,
        isPaid: false,
      });
      const configured = getLlmProviderStatus({
        provider: definition,
        config: {
          api_key: '{"type":"oauth","refresh":"r","access":"a","expires":1}',
        },
        isAuthenticated: false,
        isPaid: false,
      });

      expect(definition.authKind).toBe("subscription");
      expect(definition.badge).toBe("Subscription");
      expect(missing.configured).toBe(false);
      expect(configured.configured).toBe(true);
      expect(configured.listModels).toBeTypeOf("function");
    },
  );

  test("routes ChatGPT subscriptions through the Codex backend", () => {
    expect(provider("chatgpt").baseUrl).toBe(
      "https://chatgpt.com/backend-api/codex",
    );
  });

  test.each([
    ["moonshot", "https://api.moonshot.ai/v1"],
    ["zai", "https://api.z.ai/api/paas/v4"],
    ["deepseek", "https://api.deepseek.com"],
    ["alibaba_cloud", "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"],
    ["siliconflow", "https://api.siliconflow.com/v1"],
    ["cohere", "https://api.cohere.ai/compatibility/v1"],
    ["groq", "https://api.groq.com/openai/v1"],
    ["xai", "https://api.x.ai/v1"],
    ["together", "https://api.together.xyz/v1"],
    ["fireworks", "https://api.fireworks.ai/inference/v1"],
    ["cerebras", "https://api.cerebras.ai/v1"],
  ])("configures %s through its OpenAI-compatible endpoint", (id, baseUrl) => {
    const definition = provider(id);
    const status = getLlmProviderStatus({
      provider: definition,
      config: { api_key: "test-key" },
      isAuthenticated: false,
      isPaid: false,
    });

    expect(definition.baseUrl).toBe(baseUrl);
    expect(status.configured).toBe(true);
    expect(status.listModels).toBeTypeOf("function");
  });

  test.each(["amazon_bedrock", "google_vertex_ai"])(
    "requires both an endpoint and credentials for %s",
    (id) => {
      const definition = provider(id);
      const missingEndpoint = getLlmProviderStatus({
        provider: definition,
        config: { api_key: "test-key" },
        isAuthenticated: false,
        isPaid: false,
      });
      const configured = getLlmProviderStatus({
        provider: definition,
        config: {
          base_url: "https://provider.example.com/v1",
          api_key: "test-key",
        },
        isAuthenticated: false,
        isPaid: false,
      });

      expect(missingEndpoint.configured).toBe(false);
      expect(configured.configured).toBe(true);
      expect(configured.listModels).toBeTypeOf("function");
    },
  );

  test("uses curated models for Google Vertex AI", async () => {
    const status = getLlmProviderStatus({
      provider: provider("google_vertex_ai"),
      config: {
        base_url:
          "https://aiplatform.googleapis.com/v1/projects/project/locations/global/endpoints/openapi",
        api_key: "test-key",
      },
      isAuthenticated: false,
      isPaid: false,
    });

    const result = await status.listModels?.();

    expect(result?.models.slice(0, 3)).toEqual([
      "google/gemini-3.8-flash",
      "google/gemini-3.7-flash",
      "google/gemini-3.6-flash",
    ]);
  });

  test.each(["ollama", "lmstudio", "apple_foundation"])(
    "only configures %s when its runtime is reachable, without an API key",
    (id) => {
      const pending = getLlmProviderStatus({
        provider: provider(id),
        isAuthenticated: false,
        isPaid: false,
      });
      const unavailable = getLlmProviderStatus({
        provider: provider(id),
        isAuthenticated: false,
        isPaid: false,
        isAvailable: false,
      });
      const available = getLlmProviderStatus({
        provider: provider(id),
        isAuthenticated: false,
        isPaid: false,
        isAvailable: true,
      });

      expect(pending.configured).toBe(false);
      expect(pending.availabilityPending).toBe(true);
      expect(unavailable.configured).toBe(false);
      expect(unavailable.availabilityPending).toBeUndefined();
      expect(unavailable.listModels).toBeUndefined();
      expect(available.configured).toBe(true);
      expect(available.availabilityPending).toBeUndefined();
      expect(available.listModels).toBeTypeOf("function");
    },
  );
});
