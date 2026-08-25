import type { E2eeDeviceEnrollmentPackage } from "@anlg/plugin-db";

import { DEVICE_NAME_HEADER } from "./cloudsync-credentials";

import { env } from "~/env";

export const ENROLLMENT_REQUIRES_EXISTING_KEY_ERROR_CODE =
  "e2ee_enrollment_requires_existing_key";

export type SyncDeviceKind = "desktop" | "mobile" | "watch";

export type SyncDevice = {
  deviceFingerprint: string;
  deviceName: string | null;
  deviceKind?: SyncDeviceKind | null;
  createdAt: string;
  lastSeenAt: string;
};

export type PendingSyncDevice = {
  requestId: string;
  deviceFingerprint: string;
  deviceName: string | null;
  deviceKind?: SyncDeviceKind | null;
  publicKey: string;
  createdAt: string;
  expiresAt: string;
  status: "pending" | "sealed" | "consumed";
};

export type SyncDevices = {
  devices: SyncDevice[];
  pendingDevices: PendingSyncDevice[];
  maxDevices: number;
};

export type DeviceEnrollment = {
  requestId: string;
  expiresAt: string;
  status: "pending" | "sealed" | "consumed";
  package: E2eeDeviceEnrollmentPackage | null;
};

export class SyncDeviceRequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string | null,
  ) {
    super(message);
  }
}

export async function requestSyncDevices(
  accessToken: string,
  signal?: AbortSignal,
): Promise<SyncDevices> {
  const response = await fetch(new URL("/sync/devices", env.VITE_API_URL), {
    headers: { Authorization: `Bearer ${accessToken}` },
    signal,
  });
  if (!response.ok) {
    throw await responseError(response, "Could not load your devices.");
  }
  const body: unknown = await response.json();
  if (!isSyncDevices(body)) {
    throw new Error("The device service returned an invalid response.");
  }
  return body;
}

export async function registerDeviceEnrollment({
  accessToken,
  publicKey,
  fingerprint,
  deviceName,
  replaceFingerprint,
  signal,
}: {
  accessToken: string;
  publicKey: string;
  fingerprint: string;
  deviceName: string | null;
  replaceFingerprint?: string;
  signal?: AbortSignal;
}): Promise<DeviceEnrollment> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${accessToken}`,
    "Content-Type": "application/json",
    "x-device-fingerprint": fingerprint,
  };
  if (deviceName) {
    headers[DEVICE_NAME_HEADER] = deviceName;
  }
  const response = await fetch(
    new URL("/sync/e2ee/device-enrollments", env.VITE_API_URL),
    {
      method: "POST",
      headers,
      body: JSON.stringify({
        publicKey,
        replaceFingerprint: replaceFingerprint ?? null,
      }),
      signal,
    },
  );
  if (!response.ok) {
    throw await responseError(response, "Could not register this device.");
  }
  const body: unknown = await response.json();
  if (!isDeviceEnrollment(body)) {
    throw new Error("The enrollment service returned an invalid response.");
  }
  return body;
}

export async function sealDeviceEnrollment({
  accessToken,
  requestId,
  packageValue,
  signal,
}: {
  accessToken: string;
  requestId: string;
  packageValue: E2eeDeviceEnrollmentPackage;
  signal?: AbortSignal;
}) {
  const response = await fetch(
    new URL(
      `/sync/e2ee/device-enrollments/${encodeURIComponent(requestId)}/seal`,
      env.VITE_API_URL,
    ),
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(packageValue),
      signal,
    },
  );
  if (!response.ok) {
    throw await responseError(response, "Could not approve this device.");
  }
}

export async function consumeDeviceEnrollment({
  accessToken,
  requestId,
  publicKey,
  fingerprint,
  signal,
}: {
  accessToken: string;
  requestId: string;
  publicKey: string;
  fingerprint: string;
  signal?: AbortSignal;
}) {
  const response = await fetch(
    new URL(
      `/sync/e2ee/device-enrollments/${encodeURIComponent(requestId)}/consume`,
      env.VITE_API_URL,
    ),
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
        "x-device-fingerprint": fingerprint,
      },
      body: JSON.stringify({ publicKey }),
      signal,
    },
  );
  if (!response.ok) {
    throw await responseError(response, "Could not finish device enrollment.");
  }
}

export async function removeSyncDevice(
  accessToken: string,
  fingerprint: string,
) {
  const response = await fetch(
    new URL(
      `/sync/devices/${encodeURIComponent(fingerprint)}`,
      env.VITE_API_URL,
    ),
    {
      method: "DELETE",
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
  if (!response.ok) {
    throw await responseError(response, "Could not remove this device.");
  }
}

export async function renameSyncDevice(
  accessToken: string,
  fingerprint: string,
  deviceName: string,
) {
  const response = await fetch(
    new URL(
      `/sync/devices/${encodeURIComponent(fingerprint)}`,
      env.VITE_API_URL,
    ),
    {
      method: "PATCH",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ deviceName }),
    },
  );
  if (!response.ok) {
    throw await responseError(response, "Could not rename this device.");
  }
}

async function responseError(response: Response, fallback: string) {
  let code: string | null = null;
  let message = fallback;
  try {
    const body: unknown = await response.json();
    if (isRecord(body) && isRecord(body.error)) {
      if (typeof body.error.code === "string") {
        code = body.error.code;
      }
      if (typeof body.error.message === "string") {
        message = body.error.message;
      }
    }
  } catch {
    // Some upstream failures do not include a JSON error body.
  }
  return new SyncDeviceRequestError(message, response.status, code);
}

function isSyncDevices(value: unknown): value is SyncDevices {
  if (
    !isRecord(value) ||
    !Array.isArray(value.devices) ||
    !Array.isArray(value.pendingDevices) ||
    typeof value.maxDevices !== "number" ||
    !Number.isInteger(value.maxDevices) ||
    value.maxDevices < 1
  ) {
    return false;
  }
  return (
    value.devices.every(
      (device) =>
        isRecord(device) &&
        isFingerprint(device.deviceFingerprint) &&
        (device.deviceName === null || typeof device.deviceName === "string") &&
        isTimestamp(device.createdAt) &&
        isTimestamp(device.lastSeenAt),
    ) && value.pendingDevices.every(isPendingSyncDevice)
  );
}

function isPendingSyncDevice(value: unknown): value is PendingSyncDevice {
  return (
    isRecord(value) &&
    isRequestId(value.requestId) &&
    isFingerprint(value.deviceFingerprint) &&
    (value.deviceName === null || typeof value.deviceName === "string") &&
    isBase64url(value.publicKey, 43) &&
    isTimestamp(value.createdAt) &&
    isTimestamp(value.expiresAt) &&
    ["pending", "sealed", "consumed"].includes(String(value.status))
  );
}

function isDeviceEnrollment(value: unknown): value is DeviceEnrollment {
  if (
    !isRecord(value) ||
    !isRequestId(value.requestId) ||
    !isTimestamp(value.expiresAt) ||
    !["pending", "sealed", "consumed"].includes(String(value.status))
  ) {
    return false;
  }
  if (value.status === "sealed") {
    return isEnrollmentPackage(value.package);
  }
  return value.package === null;
}

function isEnrollmentPackage(
  value: unknown,
): value is E2eeDeviceEnrollmentPackage {
  return (
    isRecord(value) &&
    isBase64url(value.ephemeralPublicKey, 43) &&
    isBase64url(value.nonce, 32) &&
    typeof value.ciphertext === "string" &&
    value.ciphertext.length >= 64 &&
    value.ciphertext.length <= 2048 &&
    isBase64url(value.ciphertext)
  );
}

function isRequestId(value: unknown) {
  return (
    typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(
      value,
    )
  );
}

function isFingerprint(value: unknown) {
  return typeof value === "string" && /^[A-Za-z0-9_-]{8,128}$/.test(value);
}

function isTimestamp(value: unknown) {
  return typeof value === "string" && Number.isFinite(Date.parse(value));
}

function isBase64url(value: unknown, length?: number) {
  return (
    typeof value === "string" &&
    (length === undefined || value.length === length) &&
    /^[A-Za-z0-9_-]+$/.test(value)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
