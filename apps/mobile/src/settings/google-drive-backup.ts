import * as Crypto from "expo-crypto";
import { Directory, File, Paths } from "expo-file-system";
import * as SecureStore from "expo-secure-store";
import * as WebBrowser from "expo-web-browser";

import { decryptBackupFile, encryptBackupFile, executeCommand } from "@/db";
import { env } from "@/lib/env";

const CREDENTIAL_KEY = "anarlog.google-drive-backup.v1";
const DRIVE_API = "https://www.googleapis.com/drive/v3/files";
const UPLOAD_API = "https://www.googleapis.com/upload/drive/v3/files";
const BACKUP_NAME = "anarlog-mobile-backup.db.enc";
const SNAPSHOT_NAME = "anarlog-mobile-backup.db";
const BACKUP_FOLDER = "Anarlog Backups";
const BACKUP_KEY = "anarlog.google-drive-backup-key.v1";

type DriveCredential = {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
  email: string;
};

export async function getGoogleDriveAccount(): Promise<string | null> {
  return (await readCredential())?.email ?? null;
}

export async function connectGoogleDrive(): Promise<string> {
  if (!env.googleIosClientId || !env.googleIosScheme) {
    throw new Error(
      "Google Drive is not configured yet. Add the iOS OAuth Client ID and rebuild the app.",
    );
  }
  const state = randomHex(24);
  const verifier = randomHex(48);
  const challenge = base64Url(
    await Crypto.digestStringAsync(
      Crypto.CryptoDigestAlgorithm.SHA256,
      verifier,
      { encoding: Crypto.CryptoEncoding.BASE64 },
    ),
  );
  const redirectUri = `${env.googleIosScheme}:/oauthredirect`;
  const authorize = new URL("https://accounts.google.com/o/oauth2/v2/auth");
  authorize.search = new URLSearchParams({
    client_id: env.googleIosClientId,
    redirect_uri: redirectUri,
    response_type: "code",
    scope: "https://www.googleapis.com/auth/drive.file openid email",
    access_type: "offline",
    prompt: "consent",
    state,
    code_challenge: challenge,
    code_challenge_method: "S256",
  }).toString();
  const result = await WebBrowser.openAuthSessionAsync(
    authorize.toString(),
    redirectUri,
  );
  if (result.type !== "success")
    throw new Error("Google sign-in was cancelled.");
  const callback = new URL(result.url);
  if (
    callback.protocol.slice(0, -1) !== env.googleIosScheme ||
    callback.searchParams.get("state") !== state
  ) {
    throw new Error("Google sign-in returned an invalid response.");
  }
  const code = callback.searchParams.get("code");
  if (!code)
    throw new Error("Google sign-in did not return an authorization code.");
  const tokenResponse = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      client_id: env.googleIosClientId,
      code,
      code_verifier: verifier,
      grant_type: "authorization_code",
      redirect_uri: redirectUri,
    }),
  });
  const tokens = await tokenResponse.json();
  if (
    !tokenResponse.ok ||
    typeof tokens.access_token !== "string" ||
    typeof tokens.refresh_token !== "string" ||
    typeof tokens.expires_in !== "number"
  ) {
    throw new Error("Google could not finish connecting this account.");
  }
  const profileResponse = await fetch(
    "https://openidconnect.googleapis.com/v1/userinfo",
    {
      headers: { Authorization: `Bearer ${tokens.access_token}` },
    },
  );
  const profile = await profileResponse.json();
  if (!profileResponse.ok || typeof profile.email !== "string") {
    throw new Error("Google did not return the account email.");
  }
  await SecureStore.setItemAsync(
    CREDENTIAL_KEY,
    JSON.stringify({
      accessToken: tokens.access_token,
      refreshToken: tokens.refresh_token,
      expiresAt: Date.now() + tokens.expires_in * 1000,
      email: profile.email,
    } satisfies DriveCredential),
  );
  return profile.email;
}

export async function disconnectGoogleDrive(): Promise<void> {
  const credential = await readCredential();
  if (credential) {
    await fetch("https://oauth2.googleapis.com/revoke", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({ token: credential.refreshToken }),
    });
  }
  await SecureStore.deleteItemAsync(CREDENTIAL_KEY);
}

export async function backupToGoogleDrive(): Promise<void> {
  const token = await accessToken();
  const directory = new Directory(Paths.cache, "drive-backups");
  directory.create({ intermediates: true, idempotent: true });
  const snapshot = new File(directory, SNAPSHOT_NAME);
  const encrypted = new File(directory, BACKUP_NAME);
  if (snapshot.exists) snapshot.delete();
  if (encrypted.exists) encrypted.delete();
  await executeCommand("VACUUM INTO ?", [
    decodeURIComponent(snapshot.uri.replace(/^file:\/\//, "")),
  ]);
  try {
    encryptBackupFile(
      filePath(snapshot.uri),
      filePath(encrypted.uri),
      await backupKey(),
    );
    const folderId = await getBackupFolder(token);
    const existing = await listBackups(token, folderId);
    const target = existing[0];
    const form = new FormData();
    if (!target) {
      form.append(
        "metadata",
        new Blob([JSON.stringify({ name: BACKUP_NAME, parents: [folderId] })], {
          type: "application/json",
        }),
        "metadata.json",
      );
    }
    form.append("file", encrypted as unknown as Blob, BACKUP_NAME);
    const response = await fetch(
      target
        ? `${UPLOAD_API}/${encodeURIComponent(target.id)}?uploadType=multipart`
        : `${UPLOAD_API}?uploadType=multipart`,
      {
        method: target ? "PATCH" : "POST",
        headers: { Authorization: `Bearer ${token}` },
        body: form,
      },
    );
    if (!response.ok)
      throw new Error("Could not upload the backup to Google Drive.");
  } finally {
    if (snapshot.exists) snapshot.delete();
    if (encrypted.exists) encrypted.delete();
  }
}

export async function restoreFromGoogleDrive(): Promise<void> {
  const token = await accessToken();
  const backup = (await listBackups(token, await getBackupFolder(token)))[0];
  if (!backup)
    throw new Error("No Anarlog backup was found in this Google account.");
  const response = await fetch(
    `${DRIVE_API}/${encodeURIComponent(backup.id)}?alt=media`,
    {
      headers: { Authorization: `Bearer ${token}` },
    },
  );
  if (!response.ok)
    throw new Error("Could not download the Google Drive backup.");
  const bytes = new Uint8Array(await response.arrayBuffer());
  const directory = new Directory(Paths.document, "SQLite");
  directory.create({ intermediates: true, idempotent: true });
  const encrypted = new File(Paths.cache, "anarlog-drive-backup.restore.enc");
  encrypted.write(bytes);
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

async function getBackupFolder(token: string): Promise<string> {
  const query = new URLSearchParams({
    q: `name='${BACKUP_FOLDER}' and mimeType='application/vnd.google-apps.folder' and trashed=false`,
    fields: "files(id)",
    pageSize: "10",
  });
  const lookup = await fetch(`${DRIVE_API}?${query}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!lookup.ok)
    throw new Error("Could not access the Anarlog Backups folder.");
  const found = await lookup.json();
  const existing = Array.isArray(found.files) ? found.files[0]?.id : undefined;
  if (typeof existing === "string") return existing;
  const created = await fetch(DRIVE_API, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      name: BACKUP_FOLDER,
      mimeType: "application/vnd.google-apps.folder",
    }),
  });
  const folder = await created.json();
  if (!created.ok || typeof folder.id !== "string")
    throw new Error("Could not create the Anarlog Backups folder.");
  return folder.id;
}

async function listBackups(
  token: string,
  folderId: string,
): Promise<Array<{ id: string; modifiedTime: string }>> {
  const query = new URLSearchParams({
    q: `'${folderId}' in parents and name='${BACKUP_NAME}' and trashed=false`,
    fields: "files(id,modifiedTime)",
    orderBy: "modifiedTime desc",
    pageSize: "10",
  });
  const response = await fetch(`${DRIVE_API}?${query}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok)
    throw new Error("Could not access Anarlog backups in Google Drive.");
  const body = await response.json();
  return Array.isArray(body.files) ? body.files : [];
}

async function accessToken(): Promise<string> {
  const credential = await readCredential();
  if (!credential) throw new Error("Connect Google Drive first.");
  if (credential.expiresAt > Date.now() + 60_000) return credential.accessToken;
  const response = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      client_id: env.googleIosClientId,
      refresh_token: credential.refreshToken,
      grant_type: "refresh_token",
    }),
  });
  const value = await response.json();
  if (!response.ok || typeof value.access_token !== "string") {
    throw new Error(
      "Google Drive access expired. Disconnect and connect it again.",
    );
  }
  const next = {
    ...credential,
    accessToken: value.access_token as string,
    expiresAt: Date.now() + Number(value.expires_in ?? 3600) * 1000,
  };
  await SecureStore.setItemAsync(CREDENTIAL_KEY, JSON.stringify(next));
  return next.accessToken;
}

async function readCredential(): Promise<DriveCredential | null> {
  try {
    const value = JSON.parse(
      (await SecureStore.getItemAsync(CREDENTIAL_KEY)) ?? "null",
    );
    if (
      typeof value?.accessToken === "string" &&
      typeof value?.refreshToken === "string" &&
      typeof value?.expiresAt === "number" &&
      typeof value?.email === "string"
    )
      return value as DriveCredential;
  } catch {
    return null;
  }
  return null;
}

async function backupKey(): Promise<string> {
  const saved = await SecureStore.getItemAsync(BACKUP_KEY);
  if (saved && /^[0-9a-f]{64}$/i.test(saved)) return saved;
  const generated = randomHex(32);
  await SecureStore.setItemAsync(BACKUP_KEY, generated);
  return generated;
}

function filePath(uri: string): string {
  return decodeURIComponent(uri.replace(/^file:\/\//, ""));
}

function randomHex(byteCount: number): string {
  return Array.from(Crypto.getRandomBytes(byteCount), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

function base64Url(value: string): string {
  return value.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
