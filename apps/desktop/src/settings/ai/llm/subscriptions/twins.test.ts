import { describe, expect, test } from "vitest";

import {
  isFoldedSubscriptionProvider,
  shouldShowInProviderList,
  subscriptionTwinId,
} from "./twins";

describe("subscription twins", () => {
  test("maps API providers to their subscription login", () => {
    expect(subscriptionTwinId("openai")).toBe("chatgpt");
    expect(subscriptionTwinId("anthropic")).toBeUndefined();
    expect(subscriptionTwinId("xai")).toBe("grok");
    expect(subscriptionTwinId("moonshot")).toBe("kimi_code");
    expect(subscriptionTwinId("github_copilot")).toBeUndefined();
  });

  test("hides folded subscriptions until you search", () => {
    expect(isFoldedSubscriptionProvider("chatgpt")).toBe(true);
    expect(isFoldedSubscriptionProvider("github_copilot")).toBe(false);
    expect(shouldShowInProviderList("openai", "")).toBe(true);
    expect(shouldShowInProviderList("chatgpt", "")).toBe(false);
    expect(shouldShowInProviderList("chatgpt", "chat")).toBe(true);
    expect(shouldShowInProviderList("github_copilot", "")).toBe(true);
    expect(shouldShowInProviderList("anarlog", "")).toBe(false);
  });
});
