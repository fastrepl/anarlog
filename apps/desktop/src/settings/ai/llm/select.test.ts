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

  test("only configures Apple Foundation Models when available", () => {
    const pending = getLlmProviderStatus({
      provider: provider("apple_foundation"),
      isAuthenticated: false,
      isPaid: false,
    });
    const unavailable = getLlmProviderStatus({
      provider: provider("apple_foundation"),
      isAuthenticated: false,
      isPaid: false,
      isAvailable: false,
    });
    const available = getLlmProviderStatus({
      provider: provider("apple_foundation"),
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
  });
});
