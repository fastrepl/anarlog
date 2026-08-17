import { ReplicaCredentialError } from "./replica-credentials.ts";

const keyIdPattern = /^[A-Za-z0-9_-]{22}$/;

export async function claimReplicaIdentity({
  apiUrl,
  accessToken,
  keyId,
  timeoutMs = 15_000,
  fetcher = fetch,
}: {
  apiUrl: string;
  accessToken: string;
  keyId: string;
  timeoutMs?: number;
  fetcher?: typeof fetch;
}): Promise<void> {
  if (!keyIdPattern.test(keyId)) {
    throw new ReplicaCredentialError("invalid_response");
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  let response: Response;
  try {
    response = await fetcher(new URL("/sync/e2ee/identity", apiUrl), {
      method: "PUT",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ keyId }),
      signal: controller.signal,
    });
  } catch {
    throw new ReplicaCredentialError("unavailable");
  } finally {
    clearTimeout(timeout);
  }

  if (response.status === 401) {
    throw new ReplicaCredentialError("reauth_required");
  }
  if (response.status === 403) {
    throw new ReplicaCredentialError("not_entitled");
  }
  if (response.status === 409) {
    throw new ReplicaCredentialError("identity_mismatch");
  }
  if (!response.ok) {
    throw new ReplicaCredentialError("unavailable");
  }

  let value: unknown;
  try {
    value = await response.json();
  } catch {
    throw new ReplicaCredentialError("invalid_response");
  }
  if (
    !value ||
    typeof value !== "object" ||
    (value as Record<string, unknown>).keyId !== keyId
  ) {
    throw new ReplicaCredentialError("invalid_response");
  }
}
