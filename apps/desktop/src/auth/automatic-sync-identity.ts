import {
  createE2eeIdentity,
  importE2eeIdentity,
  inspectE2eeRecoveryKey,
} from "@anlg/plugin-db";

import { SyncDeviceRequestError } from "./sync-devices";

import { env } from "~/env";

export async function establishAutomaticSyncIdentity(
  accountUserId: string,
  accessToken: string,
  isCurrent: () => boolean,
): Promise<boolean> {
  const recoveryKey = await createE2eeIdentity(accountUserId);
  if (!isCurrent()) return false;
  const identity = await inspectE2eeRecoveryKey(recoveryKey);
  if (!isCurrent()) return false;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 25_000);
  try {
    const response = await fetch(
      new URL("/sync/e2ee/identity", env.VITE_API_URL),
      {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ keyId: identity.keyId }),
        signal: controller.signal,
      },
    );
    if (!isCurrent()) return false;
    if (response.status === 409) return false;
    if (!response.ok)
      throw new SyncDeviceRequestError(
        "Could not set up encrypted sync.",
        response.status,
        null,
      );
    const result = await response.json();
    if (result.keyId !== identity.keyId)
      throw new Error("Unexpected sync identity.");
    if (!isCurrent()) return false;
    await importE2eeIdentity(accountUserId, recoveryKey);
    return isCurrent();
  } finally {
    clearTimeout(timeout);
  }
}
