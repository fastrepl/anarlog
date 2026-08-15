import {
  DEVICE_NAME_HEADER,
  getDeviceIdentity,
  raceWithAbort,
  readCredentialErrorCode,
} from "./cloudsync-credentials";

import { env } from "~/env";
import { DEVICE_FINGERPRINT_HEADER } from "~/shared/utils";

export async function requestCloudsyncCredentials({
  accessToken,
  cloudsyncExtensionAvailable,
  encryptionKeyId,
  shouldStop,
  signal,
}: {
  accessToken: string;
  cloudsyncExtensionAvailable: boolean;
  encryptionKeyId: string;
  shouldStop: () => boolean;
  signal: AbortSignal;
}) {
  let response: Response | null = null;

  try {
    const device = await raceWithAbort(getDeviceIdentity(), signal);
    if (shouldStop()) {
      return { status: "stopped" as const };
    }
    const headers: Record<string, string> = {
      Authorization: `Bearer ${accessToken}`,
      "X-Anarlog-E2EE-Key-Id": encryptionKeyId,
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
          cloudsyncExtensionAvailable
            ? "/sync/token"
            : "/sync/replica/credentials",
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
    if (cloudsyncExtensionAvailable && response.status === 404) {
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
