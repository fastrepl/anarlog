import { hostname } from "@tauri-apps/plugin-os";

import {
  getE2eeIdentityStatus,
  type CloudsyncWorkspaceProjection,
} from "@anlg/plugin-db";
import { commands as miscCommands } from "@anlg/plugin-misc";

import { getStoredSettingValues } from "~/settings/queries";

type CloudsyncCredentialCore = {
  encryptionVersion: 2;
  encryptionKeyId: string;
  databaseId: string;
  token: string;
  expiresAt: string;
  workspaceId: string;
};

type LegacyCloudsyncCredentials = CloudsyncCredentialCore & {
  accountUserId?: undefined;
  personalWorkspaceId?: undefined;
  workspaces?: undefined;
};

export type ProjectedCloudsyncCredentials = CloudsyncCredentialCore &
  CloudsyncWorkspaceProjection;

export type CloudsyncCredentials =
  | LegacyCloudsyncCredentials
  | ProjectedCloudsyncCredentials;

export const DEVICE_NAME_HEADER = "x-anarlog-device-name";
export const DEVICE_LIMIT_ERROR_CODE = "sync_device_limit_reached";
export const DEVICE_LIMIT_TOAST_ID = "cloudsync-device-limit";

export type CloudsyncCredentialBlock =
  | "device_limit"
  | "identity_mismatch"
  | "keychain_access"
  | "not_entitled"
  | "reauth_required"
  | "setup_required"
  | "unavailable"
  | null;

let credentialBlock: CloudsyncCredentialBlock = null;
const credentialBlockListeners = new Set<() => void>();

export function setCredentialBlock(next: CloudsyncCredentialBlock) {
  if (credentialBlock === next) {
    return;
  }
  credentialBlock = next;
  credentialBlockListeners.forEach((listener) => listener());
}

export function getCloudsyncCredentialBlock(): CloudsyncCredentialBlock {
  return credentialBlock;
}

export function subscribeCloudsyncCredentialBlock(listener: () => void) {
  credentialBlockListeners.add(listener);
  return () => {
    credentialBlockListeners.delete(listener);
  };
}

let cachedDeviceIdentity: {
  fingerprint: string | null;
  name: string | null;
} | null = null;
let pendingStoredSettings: ReturnType<typeof getStoredSettingValues> | null =
  null;
const pendingE2eeIdentityReads = new Map<
  string,
  ReturnType<typeof getE2eeIdentityStatus>
>();

export function raceWithAbort<T>(operation: Promise<T>, signal: AbortSignal) {
  return new Promise<T>((resolve, reject) => {
    const abort = () => {
      signal.removeEventListener("abort", abort);
      reject(new Error("aborted"));
    };
    if (signal.aborted) {
      abort();
    } else {
      signal.addEventListener("abort", abort, { once: true });
    }
    operation.then(
      (value) => {
        signal.removeEventListener("abort", abort);
        resolve(value);
      },
      (error) => {
        signal.removeEventListener("abort", abort);
        reject(error);
      },
    );
  });
}

export function readStoredSettings() {
  if (pendingStoredSettings) {
    return pendingStoredSettings;
  }

  const read = getStoredSettingValues().finally(() => {
    if (pendingStoredSettings === read) {
      pendingStoredSettings = null;
    }
  });
  pendingStoredSettings = read;
  return read;
}

export function readE2eeIdentity(userId: string) {
  const pending = pendingE2eeIdentityReads.get(userId);
  if (pending) {
    return pending;
  }

  const read = getE2eeIdentityStatus(userId).finally(() => {
    if (pendingE2eeIdentityReads.get(userId) === read) {
      pendingE2eeIdentityReads.delete(userId);
    }
  });
  pendingE2eeIdentityReads.set(userId, read);
  return read;
}

export async function getDeviceIdentity() {
  if (cachedDeviceIdentity) {
    return cachedDeviceIdentity;
  }

  let fingerprint: string | null = null;
  try {
    const result = await miscCommands.getFingerprint();
    if (result.status === "ok") {
      fingerprint = result.data;
    }
  } catch {
    // Token exchange still works without a device identity.
  }

  let name: string | null = null;
  try {
    name = await hostname();
  } catch {
    // Device name is optional.
  }

  const identity = { fingerprint, name };
  // Cache only a fully resolved identity so a transiently missing
  // fingerprint or hostname is retried on the next exchange.
  if (fingerprint !== null && name !== null) {
    cachedDeviceIdentity = identity;
  }
  return identity;
}

export async function readCredentialErrorCode(
  response: Response,
): Promise<string | null> {
  try {
    const body: unknown = await response.json();
    if (
      typeof body === "object" &&
      body !== null &&
      "error" in body &&
      typeof body.error === "object" &&
      body.error !== null &&
      "code" in body.error &&
      typeof body.error.code === "string"
    ) {
      return body.error.code;
    }
  } catch {
    // Rejections without a structured body fall through to generic handling.
  }
  return null;
}

export function isCredentials(value: unknown): value is CloudsyncCredentials {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  const hasCoreCredentials =
    candidate.encryptionVersion === 2 &&
    typeof candidate.encryptionKeyId === "string" &&
    /^[A-Za-z0-9_-]{22}$/.test(candidate.encryptionKeyId) &&
    typeof candidate.databaseId === "string" &&
    candidate.databaseId.length > 0 &&
    typeof candidate.token === "string" &&
    candidate.token.length > 0 &&
    typeof candidate.expiresAt === "string" &&
    Number.isFinite(Date.parse(candidate.expiresAt)) &&
    typeof candidate.workspaceId === "string" &&
    candidate.workspaceId.length > 0;
  if (!hasCoreCredentials) {
    return false;
  }

  const projectionKeys = ["accountUserId", "personalWorkspaceId", "workspaces"];
  if (!projectionKeys.some((key) => key in candidate)) {
    return true;
  }

  if (
    typeof candidate.accountUserId !== "string" ||
    candidate.accountUserId.length === 0 ||
    typeof candidate.personalWorkspaceId !== "string" ||
    candidate.personalWorkspaceId.length === 0 ||
    candidate.personalWorkspaceId !== candidate.workspaceId ||
    candidate.accountUserId !== candidate.personalWorkspaceId ||
    !Array.isArray(candidate.workspaces) ||
    candidate.workspaces.length === 0
  ) {
    return false;
  }

  const workspaceIds = new Set<string>();
  const membershipIds = new Set<string>();
  for (const value of candidate.workspaces) {
    if (!value || typeof value !== "object") {
      return false;
    }

    const workspace = value as Record<string, unknown>;
    if (
      typeof workspace.id !== "string" ||
      workspace.id.length === 0 ||
      typeof workspace.ownerUserId !== "string" ||
      workspace.ownerUserId.length === 0 ||
      typeof workspace.kind !== "string" ||
      !["personal", "shared"].includes(workspace.kind) ||
      typeof workspace.name !== "string" ||
      typeof workspace.membershipId !== "string" ||
      workspace.membershipId.length === 0 ||
      typeof workspace.role !== "string" ||
      !["owner", "admin", "member"].includes(workspace.role) ||
      typeof workspace.membershipCreatedAt !== "string" ||
      !Number.isFinite(Date.parse(workspace.membershipCreatedAt)) ||
      typeof workspace.membershipUpdatedAt !== "string" ||
      !Number.isFinite(Date.parse(workspace.membershipUpdatedAt)) ||
      typeof workspace.createdAt !== "string" ||
      !Number.isFinite(Date.parse(workspace.createdAt)) ||
      typeof workspace.updatedAt !== "string" ||
      !Number.isFinite(Date.parse(workspace.updatedAt)) ||
      workspaceIds.has(workspace.id) ||
      membershipIds.has(workspace.membershipId)
    ) {
      return false;
    }

    workspaceIds.add(workspace.id);
    membershipIds.add(workspace.membershipId);
  }

  const personalWorkspaces = candidate.workspaces.filter(
    (workspace) => workspace.kind === "personal",
  );
  if (personalWorkspaces.length !== 1) {
    return false;
  }

  const personalWorkspace = personalWorkspaces[0]!;
  return (
    personalWorkspace.id === candidate.personalWorkspaceId &&
    personalWorkspace.ownerUserId === candidate.accountUserId &&
    personalWorkspace.role === "owner"
  );
}

export function hasWorkspaceProjection(
  credentials: CloudsyncCredentials,
): credentials is ProjectedCloudsyncCredentials {
  return credentials.accountUserId !== undefined;
}
