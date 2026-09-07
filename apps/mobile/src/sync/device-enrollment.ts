export type DeviceEnrollmentPackage = {
  ephemeralPublicKey: string;
  nonce: string;
  ciphertext: string;
};

export type DeviceEnrollment = {
  requestId: string;
  status: "pending" | "sealed" | "consumed";
  package: DeviceEnrollmentPackage | null;
};

export type DeviceEnrollmentErrorCode =
  | "device_limit"
  | "first_device"
  | "invalid_response"
  | "not_entitled"
  | "reauth_required"
  | "unavailable";

export class DeviceEnrollmentError extends Error {
  readonly code: DeviceEnrollmentErrorCode;

  constructor(code: DeviceEnrollmentErrorCode) {
    super(
      {
        device_limit: "Your plan’s sync device limit is reached.",
        first_device: "This is the first encrypted device.",
        invalid_response: "The device service returned an invalid response.",
        not_entitled: "Anarlog Pro is required for cloud sync.",
        reauth_required: "Sign in again to continue syncing.",
        unavailable: "Cloud sync is temporarily unavailable.",
      }[code],
    );
    this.code = code;
    this.name = "DeviceEnrollmentError";
  }
}

const requestIdPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const base64UrlPattern = /^[A-Za-z0-9_-]+$/;

function isPackage(value: unknown): value is DeviceEnrollmentPackage {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.ephemeralPublicKey === "string" &&
    candidate.ephemeralPublicKey.length === 43 &&
    base64UrlPattern.test(candidate.ephemeralPublicKey) &&
    typeof candidate.nonce === "string" &&
    candidate.nonce.length === 32 &&
    base64UrlPattern.test(candidate.nonce) &&
    typeof candidate.ciphertext === "string" &&
    candidate.ciphertext.length >= 64 &&
    candidate.ciphertext.length <= 2048 &&
    base64UrlPattern.test(candidate.ciphertext)
  );
}

function isEnrollment(value: unknown): value is DeviceEnrollment {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  if (
    typeof candidate.requestId !== "string" ||
    !requestIdPattern.test(candidate.requestId) ||
    !["pending", "sealed", "consumed"].includes(String(candidate.status))
  ) {
    return false;
  }
  return candidate.status === "sealed"
    ? isPackage(candidate.package)
    : candidate.package === null;
}

async function responseError(
  response: Response,
): Promise<DeviceEnrollmentError> {
  let code: string | null = null;
  try {
    const body: unknown = await response.json();
    if (body && typeof body === "object") {
      const error = (body as Record<string, unknown>).error;
      if (error && typeof error === "object") {
        const value = (error as Record<string, unknown>).code;
        code = typeof value === "string" ? value : null;
      }
    }
  } catch {
    code = null;
  }

  if (response.status === 401) {
    return new DeviceEnrollmentError("reauth_required");
  }
  if (response.status === 403) {
    return new DeviceEnrollmentError(
      code === "sync_device_limit_reached" ? "device_limit" : "not_entitled",
    );
  }
  if (
    response.status === 409 &&
    code === "e2ee_enrollment_requires_existing_key"
  ) {
    return new DeviceEnrollmentError("first_device");
  }
  return new DeviceEnrollmentError("unavailable");
}

export async function requestDeviceEnrollment({
  apiUrl,
  accessToken,
  publicKey,
  device,
  fetcher = fetch,
}: {
  apiUrl: string;
  accessToken: string;
  publicKey: string;
  device: { fingerprint?: string | null; name?: string | null };
  fetcher?: typeof fetch;
}): Promise<DeviceEnrollment> {
  if (!device.fingerprint) {
    throw new DeviceEnrollmentError("invalid_response");
  }
  let response: Response;
  try {
    response = await fetcher(new URL("/sync/e2ee/device-enrollments", apiUrl), {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
        "X-Device-Fingerprint": device.fingerprint,
        ...(device.name ? { "X-Anarlog-Device-Name": device.name } : {}),
      },
      body: JSON.stringify({ publicKey, replaceFingerprint: null }),
    });
  } catch {
    throw new DeviceEnrollmentError("unavailable");
  }
  if (!response.ok) throw await responseError(response);

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new DeviceEnrollmentError("invalid_response");
  }
  if (!isEnrollment(body)) {
    throw new DeviceEnrollmentError("invalid_response");
  }
  return body;
}

export async function consumeDeviceEnrollment({
  apiUrl,
  accessToken,
  requestId,
  publicKey,
  fingerprint,
  fetcher = fetch,
}: {
  apiUrl: string;
  accessToken: string;
  requestId: string;
  publicKey: string;
  fingerprint: string;
  fetcher?: typeof fetch;
}): Promise<void> {
  let response: Response;
  try {
    response = await fetcher(
      new URL(
        `/sync/e2ee/device-enrollments/${encodeURIComponent(requestId)}/consume`,
        apiUrl,
      ),
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/json",
          "X-Device-Fingerprint": fingerprint,
        },
        body: JSON.stringify({ publicKey }),
      },
    );
  } catch {
    throw new DeviceEnrollmentError("unavailable");
  }
  if (!response.ok) throw await responseError(response);
}
