import { execute } from "@anlg/plugin-db";

import {
  CLOUDSYNC_ACCEPTED_TRANSPORTS,
  CLOUDSYNC_TRANSPORTS_HEADER,
  DEVICE_NAME_HEADER,
  E2EE_MEMBER_PUBLIC_KEY_HEADER,
  getDeviceIdentity,
  raceWithAbort,
  readCredentialErrorCode,
} from "./cloudsync-credentials";

import { env } from "~/env";
import { DEVICE_FINGERPRINT_HEADER } from "~/shared/utils";

export async function requestCloudsyncCredentials({
  accessToken,
  accountUserId,
  cloudsyncExtensionAvailable,
  encryptionKeyId,
  memberPublicKey,
  shouldStop,
  signal,
}: {
  accessToken: string;
  accountUserId: string;
  cloudsyncExtensionAvailable: boolean;
  encryptionKeyId: string;
  memberPublicKey: string;
  shouldStop: () => boolean;
  signal: AbortSignal;
}) {
  let response: Response | null = null;

  try {
    const connections = await raceWithAbort(
      execute<{ connected: number }>(
        "SELECT 1 AS connected FROM local_library_connections WHERE account_user_id = ? LIMIT 1",
        [accountUserId],
      ),
      signal,
    );
    const useSqliteTransport =
      cloudsyncExtensionAvailable && connections.length === 0;
    const device = await raceWithAbort(getDeviceIdentity(), signal);
    if (shouldStop()) {
      return { status: "stopped" as const };
    }
    const headers: Record<string, string> = {
      Authorization: `Bearer ${accessToken}`,
      "X-Anarlog-E2EE-Key-Id": encryptionKeyId,
      [E2EE_MEMBER_PUBLIC_KEY_HEADER]: memberPublicKey,
      [CLOUDSYNC_TRANSPORTS_HEADER]: CLOUDSYNC_ACCEPTED_TRANSPORTS,
    };
    if (device.fingerprint) {
      headers[DEVICE_FINGERPRINT_HEADER] = device.fingerprint;
    }
    if (device.name) {
      headers[DEVICE_NAME_HEADER] = device.name;
    }

    response = await raceWithAbort(
      fetch(
        new URL(
          useSqliteTransport ? "/sync/token" : "/sync/replica/credentials",
          env.VITE_API_URL,
        ),
        {
          method: "POST",
          headers,
          signal,
        },
      ),
      signal,
    );
    if (useSqliteTransport && response.status === 404) {
      response = await raceWithAbort(
        fetch(new URL("/sync/replica/credentials", env.VITE_API_URL), {
          method: "POST",
          headers,
          signal,
        }),
        signal,
      );
    }

    let credentials: unknown;
    let credentialErrorCode: string | null = null;
    if (response.ok) {
      credentials = await raceWithAbort(response.json(), signal);
    } else if (response.status === 403) {
      try {
        credentialErrorCode = await raceWithAbort(
          readCredentialErrorCode(response),
          signal,
        );
      } catch {
        credentialErrorCode = null;
      }
    }

    return {
      status: "response" as const,
      response,
      credentials,
      credentialErrorCode,
    };
  } catch {
    return {
      status: "error" as const,
      responseReceived: response !== null,
    };
  }
}
