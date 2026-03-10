import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import type { CharTask } from "@hypr/api-client";
import { commands as miscCommands } from "@hypr/plugin-misc";

import {
  CHAR_TASK_HEADER,
  DEVICE_FINGERPRINT_HEADER,
  REQUEST_ID_HEADER,
  id,
} from "~/shared/utils";

let cachedFingerprint: string | null = null;

const getFingerprint = async (): Promise<string | null> => {
  if (cachedFingerprint) return cachedFingerprint;

  const result = await miscCommands.getFingerprint();
  if (result.status === "ok") {
    cachedFingerprint = result.data;
    return cachedFingerprint;
  }
  return null;
};

export const tracedFetch: typeof fetch = async (input, init) => {
  const headers = new Headers(init?.headers);
  if (!headers.has(REQUEST_ID_HEADER)) {
    headers.set(REQUEST_ID_HEADER, id());
  }

  const fingerprint = await getFingerprint();
  if (fingerprint) {
    headers.set(DEVICE_FINGERPRINT_HEADER, fingerprint);
  }

  return tauriFetch(input, { ...init, headers });
};

export function createTracedFetch(task: CharTask): typeof fetch {
  return async (input, init) => {
    const headers = new Headers(init?.headers);
    headers.set(CHAR_TASK_HEADER, task);
    return tracedFetch(input, { ...init, headers });
  };
}
