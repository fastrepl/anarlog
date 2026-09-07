import { env } from "@/lib/env";

import { parseSyncDeviceResponse } from "./sync-device-response";

export async function requestSyncDeviceList(
  token: string,
  signal?: AbortSignal,
) {
  const response = await fetch(new URL("/sync/devices", env.apiUrl), {
    headers: { Authorization: `Bearer ${token}` },
    signal,
  });
  if (!response.ok) throw new Error("Could not load your devices.");
  return parseSyncDeviceResponse(await response.json());
}
