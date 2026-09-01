import { type SubscriptionProviderId } from "./oauth";

export const API_SUBSCRIPTION_TWINS = {
  openai: "chatgpt",
  xai: "grok",
  moonshot: "kimi_code",
} as const satisfies Record<string, SubscriptionProviderId>;

export function subscriptionTwinId(
  providerId: string,
): SubscriptionProviderId | undefined {
  if (providerId in API_SUBSCRIPTION_TWINS) {
    return API_SUBSCRIPTION_TWINS[
      providerId as keyof typeof API_SUBSCRIPTION_TWINS
    ];
  }
}

export function isFoldedSubscriptionProvider(providerId: string): boolean {
  return (Object.values(API_SUBSCRIPTION_TWINS) as string[]).includes(
    providerId,
  );
}

export function shouldShowInProviderList(
  providerId: string,
  search: string,
): boolean {
  if (providerId === "anarlog") {
    return false;
  }
  if (isFoldedSubscriptionProvider(providerId) && !search.trim()) {
    return false;
  }
  return true;
}
