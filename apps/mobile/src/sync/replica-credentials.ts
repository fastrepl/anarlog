export type E2eeRecoveryKeyIdentity = {
  keyId: string;
  memberPublicKey: string;
};

export type ReplicaCredentials = {
  transport: "replica";
  encryptionVersion: 2;
  encryptionKeyId: string;
  expiresAt: string;
  workspaceId: string;
  accountUserId: string;
};

export type ReplicaCredentialErrorCode =
  | "device_limit"
  | "identity_mismatch"
  | "invalid_response"
  | "not_entitled"
  | "reauth_required"
  | "unavailable";

export class ReplicaCredentialError extends Error {
  readonly code: ReplicaCredentialErrorCode;

  constructor(code: ReplicaCredentialErrorCode) {
    super(
      {
        device_limit: "Your plan’s sync device limit is reached.",
        identity_mismatch: "This account uses a different recovery key.",
        invalid_response: "The sync service returned an invalid response.",
        not_entitled: "Anarlog Pro is required for cloud sync.",
        reauth_required: "Sign in again to continue syncing.",
        unavailable: "Cloud sync is temporarily unavailable.",
      }[code],
    );
    this.name = "ReplicaCredentialError";
    this.code = code;
  }
}

const keyIdPattern = /^[A-Za-z0-9_-]{22}$/;
const memberPublicKeyPattern = /^[A-Za-z0-9_-]{43}$/;

function isReplicaCredentials(
  value: unknown,
  accountUserId: string,
  keyId: string,
): value is ReplicaCredentials {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return (
    candidate.transport === "replica" &&
    candidate.encryptionVersion === 2 &&
    candidate.encryptionKeyId === keyId &&
    typeof candidate.encryptionKeyId === "string" &&
    keyIdPattern.test(candidate.encryptionKeyId) &&
    typeof candidate.expiresAt === "string" &&
    Number.isFinite(Date.parse(candidate.expiresAt)) &&
    candidate.workspaceId === accountUserId &&
    candidate.accountUserId === accountUserId
  );
}

async function errorCode(response: Response): Promise<string | null> {
  try {
    const body: unknown = await response.json();
    if (!body || typeof body !== "object") return null;
    const error = (body as Record<string, unknown>).error;
    if (!error || typeof error !== "object") return null;
    const code = (error as Record<string, unknown>).code;
    return typeof code === "string" ? code : null;
  } catch {
    return null;
  }
}

export async function requestReplicaCredentials({
  apiUrl,
  accessToken,
  accountUserId,
  identity,
  device,
  timeoutMs = 15_000,
  fetcher = fetch,
}: {
  apiUrl: string;
  accessToken: string;
  accountUserId: string;
  identity: E2eeRecoveryKeyIdentity;
  device?: { fingerprint?: string | null; name?: string | null };
  timeoutMs?: number;
  fetcher?: typeof fetch;
}): Promise<ReplicaCredentials> {
  if (
    !keyIdPattern.test(identity.keyId) ||
    !memberPublicKeyPattern.test(identity.memberPublicKey)
  ) {
    throw new ReplicaCredentialError("invalid_response");
  }

  const headers: Record<string, string> = {
    Authorization: `Bearer ${accessToken}`,
    "X-Anarlog-E2EE-Key-Id": identity.keyId,
    "X-Anarlog-E2EE-Member-Public-Key": identity.memberPublicKey,
  };
  if (device?.fingerprint) {
    headers["X-Device-Fingerprint"] = device.fingerprint;
  }
  if (device?.name) {
    headers["X-Anarlog-Device-Name"] = device.name;
  }

  let response: Response;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    response = await fetcher(new URL("/sync/replica/credentials", apiUrl), {
      method: "POST",
      headers,
      signal: controller.signal,
    });
  } catch {
    throw new ReplicaCredentialError("unavailable");
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    if (response.status === 401) {
      throw new ReplicaCredentialError("reauth_required");
    }
    if (response.status === 403) {
      throw new ReplicaCredentialError(
        (await errorCode(response)) === "sync_device_limit_reached"
          ? "device_limit"
          : "not_entitled",
      );
    }
    if (response.status === 409) {
      throw new ReplicaCredentialError("identity_mismatch");
    }
    throw new ReplicaCredentialError("unavailable");
  }

  let credentials: unknown;
  try {
    credentials = await response.json();
  } catch {
    throw new ReplicaCredentialError("invalid_response");
  }
  if (!isReplicaCredentials(credentials, accountUserId, identity.keyId)) {
    throw new ReplicaCredentialError("invalid_response");
  }
  return credentials;
}
