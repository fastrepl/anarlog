import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { resolveSubscriptionAccess } from "./access";
import {
  CLAUDE_OAUTH_HEADERS,
  claudeMessagesUrl,
  COPILOT_REQUEST_HEADERS,
} from "./oauth";

export function createSubscriptionFetch(
  providerId: string,
  storedApiKey: string,
): typeof fetch {
  const fetchImpl: typeof fetch = async (input, init) => {
    const headers = new Headers(init?.headers);
    const { token } = await resolveSubscriptionAccess(providerId, storedApiKey);

    if (providerId === "claude") {
      headers.delete("x-api-key");
      headers.set("Authorization", `Bearer ${token}`);
      for (const [key, value] of Object.entries(CLAUDE_OAUTH_HEADERS)) {
        if (!headers.has(key)) {
          headers.set(key, value);
        }
      }
      const url = claudeMessagesUrl(requestUrl(input));
      return tauriFetch(url, { ...init, headers });
    }

    headers.set("Authorization", `Bearer ${token}`);
    if (providerId === "github_copilot") {
      for (const [key, value] of Object.entries(COPILOT_REQUEST_HEADERS)) {
        headers.set(key, value);
      }
    }

    return tauriFetch(input as RequestInfo | URL, { ...init, headers });
  };

  return fetchImpl;
}

function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
}
