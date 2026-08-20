import {
  isOAuthCredentialFresh,
  type OAuthCredential,
  parseOAuthCredential,
  serializeOAuthCredential,
} from "./credential";
import {
  CHATGPT_API_BASE_URL,
  isSubscriptionProviderId,
  refreshOAuthCredential,
  type SubscriptionProviderId,
} from "./oauth";

import { getStoredAiProvider, setAiProvider } from "~/settings/providers";

const refreshLocks = new Map<string, Promise<OAuthCredential>>();

export async function resolveSubscriptionAccess(
  providerId: string,
  storedApiKey: string,
): Promise<{ token: string; credential?: OAuthCredential }> {
  if (!isSubscriptionProviderId(providerId) || providerId === "kimi_code") {
    return { token: storedApiKey };
  }

  const credential = parseOAuthCredential(storedApiKey);
  if (!credential) {
    return { token: storedApiKey };
  }

  if (isOAuthCredentialFresh(credential)) {
    return { token: credential.access, credential };
  }

  const next = await refreshStoredOAuth(providerId, credential);
  return { token: next.access, credential: next };
}

async function refreshStoredOAuth(
  providerId: SubscriptionProviderId,
  credential: OAuthCredential,
): Promise<OAuthCredential> {
  const existing = refreshLocks.get(providerId);
  if (existing) {
    return existing;
  }

  const pending = (async () => {
    const latest = await getStoredAiProvider("llm", providerId);
    const latestCredential = parseOAuthCredential(latest?.api_key ?? "");
    const current =
      latestCredential && isOAuthCredentialFresh(latestCredential)
        ? latestCredential
        : await refreshOAuthCredential(
            providerId,
            latestCredential ?? credential,
          );

    if (
      serializeOAuthCredential(current) !==
      serializeOAuthCredential(latestCredential ?? credential)
    ) {
      await setAiProvider("llm", providerId, {
        api_key: serializeOAuthCredential(current),
        base_url:
          providerId === "chatgpt" ? CHATGPT_API_BASE_URL : latest?.base_url,
      });
    }

    return current;
  })();

  refreshLocks.set(providerId, pending);
  try {
    return await pending;
  } finally {
    refreshLocks.delete(providerId);
  }
}
