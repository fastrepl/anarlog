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

  test("keeps local providers active without API keys", () => {
    const status = getLlmProviderStatus({
      provider: provider("ollama"),
      isAuthenticated: false,
      isPaid: false,
    });

    expect(status.configured).toBe(true);
    expect(status.listModels).toBeTypeOf("function");
  });
});
