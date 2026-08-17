import * as Crypto from "expo-crypto";
import * as Device from "expo-device";
import * as SecureStore from "expo-secure-store";

import {
  bootstrapE2eeReplica,
  generateE2eeRecoveryKey,
  getSyncStatus,
  inspectE2eeRecoveryKey,
  stopSync,
  syncNow,
} from "@/db/client";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { MobileSyncController } from "@/sync/controller";
import { claimReplicaIdentity } from "@/sync/identity";

const keychainOptions: SecureStore.SecureStoreOptions = {
  keychainService: "so.anarlog.mobile.cloudsync",
  keychainAccessible: SecureStore.WHEN_UNLOCKED,
};
const deviceFingerprintKey = "anarlog.sync.device-fingerprint";
const accountIdPattern = /^[A-Za-z0-9_-]{1,128}$/;
const fingerprintPattern = /^[A-Za-z0-9_-]{8,128}$/;

function recoveryKeyStorageKey(accountUserId: string): string {
  if (!accountIdPattern.test(accountUserId)) {
    throw new Error("Unexpected account identity.");
  }
  return `anarlog.sync.recovery.${accountUserId}`;
}

async function getDeviceFingerprint(): Promise<string> {
  const stored = await SecureStore.getItemAsync(
    deviceFingerprintKey,
    keychainOptions,
  );
  if (stored && fingerprintPattern.test(stored)) {
    return stored;
  }

  const fingerprint = Crypto.randomUUID();
  await SecureStore.setItemAsync(
    deviceFingerprintKey,
    fingerprint,
    keychainOptions,
  );
  return fingerprint;
}

const controller = new MobileSyncController({
  readRecoveryKey: async (accountUserId) =>
    await SecureStore.getItemAsync(
      recoveryKeyStorageKey(accountUserId),
      keychainOptions,
    ),
  saveRecoveryKey: async (accountUserId, recoveryKey) => {
    await SecureStore.setItemAsync(
      recoveryKeyStorageKey(accountUserId),
      recoveryKey,
      keychainOptions,
    );
  },
  deleteRecoveryKey: async (accountUserId) => {
    await SecureStore.deleteItemAsync(
      recoveryKeyStorageKey(accountUserId),
      keychainOptions,
    );
  },
  generateRecoveryKey: generateE2eeRecoveryKey,
  inspectRecoveryKey: inspectE2eeRecoveryKey,
  claimIdentity: async (session, keyId) => {
    await claimReplicaIdentity({
      apiUrl: session.apiUrl,
      accessToken: session.accessToken,
      keyId,
    });
  },
  getDevice: async () => ({
    fingerprint: await getDeviceFingerprint(),
    name: Device.deviceName ?? Device.modelName,
  }),
  bootstrap: async (session, recoveryKeyCode, device) =>
    await bootstrapE2eeReplica({
      apiUrl: session.apiUrl,
      accessToken: session.accessToken,
      accountUserId: session.accountUserId,
      recoveryKeyCode,
      device,
    }),
  stop: stopSync,
  syncNow,
  getStatus: getSyncStatus,
  reportError: (error, operation) => {
    captureOperationalError(error, { operation, level: "warning" });
  },
});

export const subscribeMobileSync = controller.subscribe;
export const getMobileSyncSnapshot = controller.getSnapshot;

export function activateMobileSync({
  accessToken,
  accountUserId,
}: {
  accessToken: string;
  accountUserId: string;
}): () => void {
  return controller.activate({
    apiUrl: env.apiUrl,
    accessToken,
    accountUserId,
  });
}

export function suspendMobileSync(): void {
  controller.suspend();
}

export function retryMobileSync(): void {
  controller.retry();
}

export async function generateMobileRecoveryKey(): Promise<string> {
  return await controller.generateRecoveryKey();
}

export async function confirmMobileRecoveryKey(
  recoveryKey: string,
): Promise<void> {
  await controller.confirmRecoveryKey(recoveryKey);
}

export async function importMobileRecoveryKey(
  recoveryKey: string,
): Promise<void> {
  await controller.importRecoveryKey(recoveryKey);
}

export async function syncMobileNow(): Promise<void> {
  await controller.syncNow();
}
