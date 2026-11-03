import * as Crypto from "expo-crypto";
import { Directory, File, Paths } from "expo-file-system";
import * as SecureStore from "expo-secure-store";
import { AppState } from "react-native";

import { decryptBackupFile, encryptBackupFile, executeCommand } from "@/db";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";

const BACKUP_KEY = "anarlog.r2-backup-key.v1";
const SNAPSHOT_NAME = "anarlog-r2-snapshot.db";
const ENCRYPTED_NAME = "anarlog-r2-snapshot.db.enc";
const BACKUP_INTERVAL_MS = 30 * 60 * 1000;

export async function backupDatabaseToR2(accessToken: string): Promise<void> {
  const directory = new Directory(Paths.cache, "r2-backups");
  directory.create({ intermediates: true, idempotent: true });
  const snapshot = new File(directory, SNAPSHOT_NAME);
  const encrypted = new File(directory, ENCRYPTED_NAME);
  if (snapshot.exists) snapshot.delete();
  if (encrypted.exists) encrypted.delete();
  await executeCommand("VACUUM INTO ?", [filePath(snapshot.uri)]);
  try {
    encryptBackupFile(
      filePath(snapshot.uri),
      filePath(encrypted.uri),
      await backupKey(),
    );
    const response = await fetch(
      new URL("/database-backups/upload", env.r2BackupWorkerUrl),
      {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/octet-stream",
        },
        body: encrypted as unknown as Blob,
      },
    );
    if (!response.ok)
      throw new Error("Could not upload the encrypted R2 backup.");
  } finally {
    if (snapshot.exists) snapshot.delete();
    if (encrypted.exists) encrypted.delete();
  }
}

export async function restoreDatabaseFromR2(
  accessToken: string,
): Promise<void> {
  const encrypted = await File.downloadFileAsync(
    new URL("/database-backups/download", env.r2BackupWorkerUrl).toString(),
    new File(Paths.cache, "anarlog-r2-restore.enc"),
    {
      headers: { Authorization: `Bearer ${accessToken}` },
      idempotent: true,
    },
  );
  const directory = new Directory(Paths.document, "SQLite");
  directory.create({ intermediates: true, idempotent: true });
  const incoming = new File(directory, "anarlog.db.restore");
  try {
    decryptBackupFile(
      filePath(encrypted.uri),
      filePath(incoming.uri),
      await backupKey(),
    );
  } catch (error) {
    if (incoming.exists) incoming.delete();
    throw error;
  } finally {
    if (encrypted.exists) encrypted.delete();
  }
}

export function activateR2DatabaseBackup(accessToken: string): {
  resume: () => void;
  pause: () => void;
  stop: () => void;
} {
  let active = false;
  let stopped = false;
  let running = false;
  let lastSuccessfulBackupAt = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const schedule = (delay = BACKUP_INTERVAL_MS) => {
    clearTimeout(timer);
    if (active && !stopped) timer = setTimeout(() => void run(), delay);
  };
  const run = async () => {
    if (!active || stopped || running || AppState.currentState !== "active")
      return;
    running = true;
    try {
      await backupDatabaseToR2(accessToken);
      lastSuccessfulBackupAt = Date.now();
    } catch (error) {
      captureOperationalError(error, {
        operation: "r2_database_backup",
        level: "warning",
      });
    } finally {
      running = false;
      schedule();
    }
  };
  return {
    resume: () => {
      if (!stopped) {
        active = true;
        if (!running) {
          clearTimeout(timer);
          timer = undefined;
          const sinceLastBackup = Date.now() - lastSuccessfulBackupAt;
          if (lastSuccessfulBackupAt && sinceLastBackup < BACKUP_INTERVAL_MS)
            schedule(BACKUP_INTERVAL_MS - sinceLastBackup);
          else void run();
        }
      }
    },
    pause: () => {
      active = false;
      clearTimeout(timer);
      timer = undefined;
    },
    stop: () => {
      stopped = true;
      active = false;
      clearTimeout(timer);
      timer = undefined;
    },
  };
}

async function backupKey(): Promise<string> {
  const saved = await SecureStore.getItemAsync(BACKUP_KEY);
  if (saved && /^[0-9a-f]{64}$/i.test(saved)) return saved;
  const generated = Array.from(Crypto.getRandomBytes(32), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
  await SecureStore.setItemAsync(BACKUP_KEY, generated);
  return generated;
}

function filePath(uri: string): string {
  return decodeURIComponent(uri.replace(/^file:\/\//, ""));
}
