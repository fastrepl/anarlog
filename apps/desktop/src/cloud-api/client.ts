import { fetch } from "@tauri-apps/plugin-http";

import { commands as localApiCommands } from "@anlg/plugin-local-api";

import { supabase } from "~/auth/client";
import { env } from "~/env";

const REQUEST_TIMEOUT_MS = 30_000;

export type CloudApiSettings = {
  enabled: boolean;
  updated_at: string | null;
};

export type CloudApiKey = {
  id: string;
  name: string;
  key_prefix: string;
  created_at: string;
  last_used_at: string | null;
};

export type CreatedCloudApiKey = CloudApiKey & { key: string };

type ApiErrorEnvelope = {
  error?: {
    code?: string;
    message?: string;
  };
};

export class CloudApiClientError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "CloudApiClientError";
  }
}

let cachedEnabled: boolean | null = null;
let cachedUserId: string | null = null;
let backfillRetryTimer: ReturnType<typeof setTimeout> | null = null;
type SnapshotIntent = { kind: "delete" | "upsert" };
const snapshotIntents = new Map<string, SnapshotIntent>();
const snapshotSyncs = new Map<string, Promise<void>>();
let snapshotRequestQueue = Promise.resolve();

function setSnapshotIntent(sessionId: string, kind: SnapshotIntent["kind"]) {
  const intent = { kind };
  snapshotIntents.set(sessionId, intent);
  return intent;
}

function clearSnapshotIntent(sessionId: string, expected: SnapshotIntent) {
  if (snapshotIntents.get(sessionId) === expected) {
    snapshotIntents.delete(sessionId);
  }
}

function hasSnapshotIntent(sessionId: string, kind: SnapshotIntent["kind"]) {
  return snapshotIntents.get(sessionId)?.kind === kind;
}

async function session() {
  if (!supabase) {
    throw new CloudApiClientError(
      "not_configured",
      "Cloud API is unavailable in this build.",
    );
  }
  const { data, error } = await supabase.auth.getSession();
  if (error) {
    throw error;
  }
  if (!data.session || data.session.user.is_anonymous === true) {
    throw new CloudApiClientError(
      "unauthorized",
      "Sign in to manage Cloud API & Connectors.",
    );
  }
  if (cachedUserId !== data.session.user.id) {
    cachedUserId = data.session.user.id;
    cachedEnabled = null;
  }
  return data.session;
}

async function request(
  path: string,
  init: RequestInit = {},
  retryAuth = true,
  retryRateLimit = true,
  expectedUserId?: string,
): Promise<Response> {
  const current = await session();
  if (expectedUserId && current.user.id !== expectedUserId) {
    throw new CloudApiClientError(
      "unauthorized",
      "The signed-in account changed during the request.",
    );
  }
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${current.access_token}`);
  if (init.body) {
    headers.set("Content-Type", "application/json");
  }
  const response = await fetchWithTimeout(
    new URL(path, env.VITE_API_URL).toString(),
    {
      ...init,
      headers,
    },
  );
  if (response.status === 401 && retryAuth && supabase) {
    const { error } = await supabase.auth.refreshSession();
    if (!error) {
      return request(path, init, false, retryRateLimit, expectedUserId);
    }
  }
  if (response.status === 429 && retryRateLimit) {
    const retryAfter = Number(response.headers.get("retry-after") ?? "1");
    await new Promise((resolve) =>
      setTimeout(resolve, Math.max(1, retryAfter) * 1000),
    );
    return request(path, init, retryAuth, false, expectedUserId);
  }
  if (!response.ok) {
    const body = (await response
      .json()
      .catch(() => null)) as ApiErrorEnvelope | null;
    throw new CloudApiClientError(
      body?.error?.code ?? `http_${response.status}`,
      body?.error?.message ?? `Cloud API request failed (${response.status}).`,
    );
  }
  return response;
}

async function fetchWithTimeout(url: string, init: RequestInit) {
  const controller = new AbortController();
  const externalSignal = init.signal;
  const abortFromCaller = () => controller.abort(externalSignal?.reason);
  if (externalSignal?.aborted) {
    abortFromCaller();
  } else {
    externalSignal?.addEventListener("abort", abortFromCaller, { once: true });
  }
  let timeout: ReturnType<typeof setTimeout> | null = null;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timeout = setTimeout(() => {
      const error = new CloudApiClientError(
        "request_timeout",
        "Cloud API request timed out.",
      );
      controller.abort(error);
      reject(error);
    }, REQUEST_TIMEOUT_MS);
  });

  try {
    return await Promise.race([
      fetch(url, { ...init, signal: controller.signal }),
      timeoutPromise,
    ]);
  } finally {
    if (timeout !== null) {
      clearTimeout(timeout);
    }
    externalSignal?.removeEventListener("abort", abortFromCaller);
  }
}

async function json<T>(
  path: string,
  init?: RequestInit,
  expectedUserId?: string,
): Promise<T> {
  const response = await request(path, init, true, true, expectedUserId);
  return response.json() as Promise<T>;
}

export async function getCloudApiSettings(): Promise<CloudApiSettings> {
  const current = await session();
  const settings = await json<CloudApiSettings>(
    "/v1/cloud-api/settings",
    undefined,
    current.user.id,
  );
  cachedEnabled = settings.enabled;
  return settings;
}

export async function setCloudApiEnabled(
  enabled: boolean,
): Promise<CloudApiSettings> {
  const current = await session();
  const settings = await json<CloudApiSettings>(
    "/v1/cloud-api/settings",
    {
      method: "PUT",
      body: JSON.stringify({ enabled }),
    },
    current.user.id,
  );
  cachedEnabled = settings.enabled;
  if (!enabled) {
    removeStoredValue(backfillKey(current.user.id));
    removeStoredValue(pendingChangesKey(current.user.id));
  }
  return settings;
}

export async function listCloudApiKeys(): Promise<CloudApiKey[]> {
  const current = await session();
  return json("/v1/cloud-api/keys", undefined, current.user.id);
}

export async function createCloudApiKey(
  name: string,
): Promise<CreatedCloudApiKey> {
  const current = await session();
  return json(
    "/v1/cloud-api/keys",
    {
      method: "POST",
      body: JSON.stringify({ name }),
    },
    current.user.id,
  );
}

export async function revokeCloudApiKey(id: string): Promise<void> {
  const current = await session();
  await request(
    `/v1/cloud-api/keys/${encodeURIComponent(id)}`,
    { method: "DELETE" },
    true,
    true,
    current.user.id,
  );
}

async function cloudApiEnabled(userId: string): Promise<boolean> {
  const current = await session();
  if (current.user.id !== userId) {
    throw new CloudApiClientError(
      "unauthorized",
      "The signed-in account changed during the request.",
    );
  }
  if (cachedEnabled !== null) {
    return cachedEnabled;
  }
  return (await getCloudApiSettings()).enabled;
}

export async function syncCloudApiSnapshot(sessionId: string): Promise<void> {
  const intent = setSnapshotIntent(sessionId, "upsert");
  try {
    const current = await session();
    await syncCloudApiSnapshotForUser(sessionId, current.user.id);
  } finally {
    clearSnapshotIntent(sessionId, intent);
  }
}

async function syncCloudApiSnapshotForUser(
  sessionId: string,
  userId: string,
): Promise<void> {
  const key = `${userId}:${sessionId}`;
  const previous = snapshotSyncs.get(key);
  const sync = (previous ? previous.catch(() => {}) : Promise.resolve()).then(
    () => performCloudApiSnapshotSync(sessionId, userId),
  );
  snapshotSyncs.set(key, sync);
  try {
    await sync;
  } finally {
    if (snapshotSyncs.get(key) === sync) {
      snapshotSyncs.delete(key);
    }
  }
}

async function performCloudApiSnapshotSync(
  sessionId: string,
  userId: string,
): Promise<void> {
  if (!(await cloudApiEnabled(userId))) {
    removePendingChange(userId, sessionId, "upserts");
    return;
  }
  const result = await localApiCommands.getCloudSnapshot(sessionId);
  if (result.status === "error") {
    if (await localSnapshotIsMissing(sessionId)) {
      const deleteIntent = setSnapshotIntent(sessionId, "delete");
      cancelScheduledSnapshotSync(sessionId);
      addPendingChange(userId, sessionId, "deletes");
      try {
        await deleteCloudApiSnapshotForUser(sessionId, userId);
        removePendingChange(userId, sessionId, "upserts");
      } finally {
        clearSnapshotIntent(sessionId, deleteIntent);
      }
      return;
    }
    throw new CloudApiClientError("local_snapshot_unavailable", result.error);
  }
  if (hasSnapshotIntent(sessionId, "delete")) {
    return;
  }
  const uploaded = await enqueueSnapshotRequest(async () => {
    if (hasSnapshotIntent(sessionId, "delete")) {
      return false;
    }
    await request(
      `/v1/sync-snapshots/${encodeURIComponent(sessionId)}`,
      {
        method: "PUT",
        body: JSON.stringify(result.data),
      },
      true,
      true,
      userId,
    );
    return true;
  });
  if (uploaded) {
    removePendingChange(userId, sessionId, "upserts");
  }
}

export async function deleteCloudApiSnapshot(sessionId: string): Promise<void> {
  const intent = setSnapshotIntent(sessionId, "delete");
  cancelScheduledSnapshotSync(sessionId);
  try {
    const current = await session();
    addPendingChange(current.user.id, sessionId, "deletes");
    await waitForSnapshotSync(sessionId, current.user.id);
    if (snapshotIntents.get(sessionId) !== intent) {
      return;
    }
    await deleteCloudApiSnapshotForUser(sessionId, current.user.id);
  } finally {
    clearSnapshotIntent(sessionId, intent);
  }
}

async function deleteCloudApiSnapshotForUser(
  sessionId: string,
  userId: string,
): Promise<void> {
  if (!(await cloudApiEnabled(userId))) {
    removePendingChange(userId, sessionId, "deletes");
    return;
  }
  await enqueueSnapshotRequest(() =>
    request(
      `/v1/sync-snapshots/${encodeURIComponent(sessionId)}`,
      { method: "DELETE" },
      true,
      true,
      userId,
    ),
  );
  removePendingChange(userId, sessionId, "deletes");
}

export async function backfillCloudApiSnapshots(): Promise<number> {
  const current = await session();
  if (!(await cloudApiEnabled(current.user.id))) {
    return 0;
  }
  const result = await localApiCommands.listCloudSnapshotIds();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  let uploaded = 0;
  let retryableError: unknown;
  for (const sessionId of result.data) {
    if (hasSnapshotIntent(sessionId, "delete")) {
      continue;
    }
    try {
      await syncCloudApiSnapshotForUser(sessionId, current.user.id);
      uploaded += 1;
    } catch (error) {
      if (shouldRetry(error)) {
        addPendingChange(current.user.id, sessionId, "upserts");
        retryableError ??= error;
      }
      reportBackgroundError(error);
    }
  }
  setStoredValue(backfillKey(current.user.id), "complete");
  if (retryableError) {
    throw retryableError;
  }
  return uploaded;
}

export async function initializeCloudApiBackfill(): Promise<void> {
  const current = await session();
  const settings = await getCloudApiSettings();
  if (!settings.enabled) {
    return;
  }
  await flushPendingChanges("deletes");
  if (getStoredValue(backfillKey(current.user.id)) !== "complete") {
    await backfillCloudApiSnapshots();
  } else {
    await flushPendingChanges("upserts");
  }
}

export function syncCloudApiSnapshotBestEffort(sessionId: string): void {
  const intent = setSnapshotIntent(sessionId, "upsert");
  void session()
    .then(async (current) => {
      if (snapshotIntents.get(sessionId) !== intent) {
        return;
      }
      addPendingChange(current.user.id, sessionId, "upserts");
      try {
        await syncCloudApiSnapshotForUser(sessionId, current.user.id);
      } catch (error) {
        if (shouldRetry(error)) {
          addPendingChange(current.user.id, sessionId, "upserts");
        }
        reportBackgroundError(error);
      }
    })
    .catch(reportBackgroundError)
    .finally(() => clearSnapshotIntent(sessionId, intent));
}

export function deleteCloudApiSnapshotBestEffort(sessionId: string): void {
  const intent = setSnapshotIntent(sessionId, "delete");
  cancelScheduledSnapshotSync(sessionId);
  void session()
    .then(async (current) => {
      if (snapshotIntents.get(sessionId) !== intent) {
        return;
      }
      addPendingChange(current.user.id, sessionId, "deletes");
      try {
        await waitForSnapshotSync(sessionId, current.user.id);
        if (snapshotIntents.get(sessionId) !== intent) {
          return;
        }
        await deleteCloudApiSnapshotForUser(sessionId, current.user.id);
      } catch (error) {
        if (shouldRetry(error)) {
          addPendingChange(current.user.id, sessionId, "deletes");
        }
        reportBackgroundError(error);
      }
    })
    .catch(reportBackgroundError)
    .finally(() => clearSnapshotIntent(sessionId, intent));
}

export function scheduleCloudApiSnapshotSync(sessionId: string): void {
  const intent = setSnapshotIntent(sessionId, "upsert");
  void session()
    .then((current) => {
      if (snapshotIntents.get(sessionId) !== intent) {
        return;
      }
      addPendingChange(current.user.id, sessionId, "upserts");
      const timerKey = `${current.user.id}:${sessionId}`;
      window.clearTimeout(snapshotTimers.get(timerKey)?.timeoutId);
      const timeoutId = window.setTimeout(() => {
        snapshotTimers.delete(timerKey);
        if (snapshotIntents.get(sessionId) !== intent) {
          return;
        }
        void syncCloudApiSnapshotForUser(sessionId, current.user.id)
          .catch((error: unknown) => {
            if (shouldRetry(error)) {
              addPendingChange(current.user.id, sessionId, "upserts");
            }
            reportBackgroundError(error);
          })
          .finally(() => clearSnapshotIntent(sessionId, intent));
      }, 1500);
      snapshotTimers.set(timerKey, { sessionId, timeoutId });
    })
    .catch((error) => {
      clearSnapshotIntent(sessionId, intent);
      reportBackgroundError(error);
    });
}

export function scheduleCloudApiBackfillRetry(): void {
  if (backfillRetryTimer !== null) {
    return;
  }
  backfillRetryTimer = setTimeout(() => {
    backfillRetryTimer = null;
    void initializeCloudApiBackfill().catch(() => {
      scheduleCloudApiBackfillRetry();
    });
  }, 30_000);
}

function backfillKey(userId: string) {
  return `anarlog.cloud-api-backfill.v1.${userId}`;
}

const snapshotTimers = new Map<
  string,
  { sessionId: string; timeoutId: number }
>();

function cancelScheduledSnapshotSync(sessionId: string) {
  for (const [timerKey, timer] of snapshotTimers) {
    if (timer.sessionId !== sessionId) {
      continue;
    }
    window.clearTimeout(timer.timeoutId);
    snapshotTimers.delete(timerKey);
  }
}

async function localSnapshotIsMissing(sessionId: string) {
  try {
    const result = await localApiCommands.listCloudSnapshotIds();
    return result.status === "ok" && !result.data.includes(sessionId);
  } catch {
    return false;
  }
}

async function waitForSnapshotSync(sessionId: string, userId: string) {
  const key = `${userId}:${sessionId}`;
  while (true) {
    const sync = snapshotSyncs.get(key);
    if (!sync) {
      return;
    }
    try {
      await sync;
    } catch {
      // The delete still needs to run after a failed upload.
    }
    if (snapshotSyncs.get(key) === sync) {
      return;
    }
  }
}

type PendingChanges = {
  upserts: string[];
  deletes: string[];
};

function pendingChangesKey(userId: string) {
  return `anarlog.cloud-api-pending.v1.${userId}`;
}

function readPendingChanges(userId: string): PendingChanges {
  try {
    const value = JSON.parse(
      getStoredValue(pendingChangesKey(userId)) ?? "{}",
    ) as Partial<PendingChanges>;
    return {
      upserts: Array.isArray(value.upserts) ? value.upserts : [],
      deletes: Array.isArray(value.deletes) ? value.deletes : [],
    };
  } catch {
    return { upserts: [], deletes: [] };
  }
}

function updatePendingChanges(
  userId: string,
  update: (pending: PendingChanges) => PendingChanges,
) {
  try {
    const pending = update(readPendingChanges(userId));
    setStoredValue(pendingChangesKey(userId), JSON.stringify(pending));
  } catch {
    return;
  }
}

function addPendingChange(
  userId: string,
  sessionId: string,
  kind: keyof PendingChanges,
) {
  updatePendingChanges(userId, (pending) => {
    if (
      kind === "upserts" &&
      (hasSnapshotIntent(sessionId, "delete") ||
        pending.deletes.includes(sessionId))
    ) {
      return pending;
    }
    const other = kind === "upserts" ? "deletes" : "upserts";
    return {
      ...pending,
      [kind]: [...new Set([...pending[kind], sessionId])],
      [other]: pending[other].filter((id) => id !== sessionId),
    };
  });
}

function removePendingChange(
  userId: string,
  sessionId: string,
  kind: keyof PendingChanges,
) {
  updatePendingChanges(userId, (pending) => ({
    ...pending,
    [kind]: pending[kind].filter((id) => id !== sessionId),
  }));
}

async function flushPendingChanges(kind: keyof PendingChanges) {
  const current = await session();
  const pending = readPendingChanges(current.user.id);
  for (const sessionId of pending[kind]) {
    if (kind === "deletes") {
      await deleteCloudApiSnapshotForUser(sessionId, current.user.id);
    } else {
      await syncCloudApiSnapshotForUser(sessionId, current.user.id);
    }
  }
}

function shouldRetry(error: unknown) {
  return !(
    error instanceof CloudApiClientError &&
    [
      "subscription_required",
      "cloud_api_not_enabled",
      "invalid_request",
      "not_configured",
    ].includes(error.code)
  );
}

function enqueueSnapshotRequest(operation: () => Promise<unknown>) {
  const queuedRequest = snapshotRequestQueue.catch(() => {}).then(operation);
  snapshotRequestQueue = queuedRequest.then(
    () => undefined,
    () => undefined,
  );
  return queuedRequest;
}

function getStoredValue(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function setStoredValue(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    return;
  }
}

function removeStoredValue(key: string) {
  try {
    localStorage.removeItem(key);
  } catch {
    return;
  }
}

function reportBackgroundError(error: unknown) {
  if (
    error instanceof CloudApiClientError &&
    ["unauthorized", "subscription_required", "cloud_api_not_enabled"].includes(
      error.code,
    )
  ) {
    return;
  }
  console.error(
    "[cloud-api] background sync failed",
    error instanceof CloudApiClientError
      ? `${error.name} [${error.code}]: ${error.message}`
      : error instanceof Error
        ? `${error.name}: ${error.message}`
        : typeof error,
  );
}
