import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { Effect, pipe } from "effect";

import { REQUEST_TIMEOUT } from "./list-common";
import { getLMStudioNativeModelsUrl } from "./list-lmstudio";

export async function checkOllamaAvailability(
  baseUrl: string,
): Promise<boolean> {
  return checkEndpoint(() => {
    const url = new URL(baseUrl);
    const path = url.pathname.replace(/\/+$/, "");
    url.pathname = `${path.endsWith("/v1") ? path.slice(0, -3) : path}/api/version`;

    return {
      url: url.toString(),
      headers: { Origin: url.origin },
    };
  });
}

export async function checkLMStudioAvailability(
  baseUrl: string,
  apiKey: string,
): Promise<boolean> {
  return checkEndpoint(() => {
    const headers: Record<string, string> = {};
    if (apiKey.trim()) {
      headers.Authorization = `Bearer ${apiKey.trim()}`;
    }

    return { url: getLMStudioNativeModelsUrl(baseUrl), headers };
  });
}

function checkEndpoint(
  getRequest: () => { url: string; headers: Record<string, string> },
): Promise<boolean> {
  return pipe(
    Effect.tryPromise(async () => {
      const { url, headers } = getRequest();
      const response = await tauriFetch(url, { method: "GET", headers });
      return response.ok;
    }),
    Effect.timeout(REQUEST_TIMEOUT),
    Effect.catchAll(() => Effect.succeed(false)),
    Effect.runPromise,
  );
}
