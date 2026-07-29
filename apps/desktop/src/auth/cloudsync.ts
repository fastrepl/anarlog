import { t } from "@lingui/core/macro";
import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { hostname } from "@tauri-apps/plugin-os";

import type { CloudsyncWorkspaceProjection } from "@anlg/plugin-db";
import {
  bindCloudsyncAccount,
  configureCloudsyncToken,
  execute,
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  isCloudsyncActivityDeferredError,
  suspendCloudsync,
  suspendCloudsyncAfterAuthLoss,
  suspendCloudsyncForSignOut,
} from "@anlg/plugin-db";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as miscCommands } from "@anlg/plugin-misc";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

import { env } from "~/env";
import { getStoredSettingValues } from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { DEVICE_FINGERPRINT_HEADER } from "~/shared/utils";

const REFRESH_LEAD_MS = 2 * 60 * 1000;
const RETRY_DELAY_MS = 60 * 1000;
const ACTIVITY_RETRY_DELAY_MS = 5 * 1000;
const MIN_REFRESH_DELAY_MS = 1000;
const EXCHANGE_TIMEOUT_MS = 25 * 1000;
const EVICTION_RETRY_DELAY_MS = 30 * 1000;
const CLOUDSYNC_TEARDOWN_TIMEOUT_MS = 2 * 1000;

export type CloudsyncAuthChangeResult = "ok" | "account_mismatch";

type CloudsyncAccountMismatchHandler = () => Promise<void>;

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

type ProjectedCloudsyncCredentials = CloudsyncCredentialCore &
  CloudsyncWorkspaceProjection;

type CloudsyncCredentials =
  | LegacyCloudsyncCredentials
  | ProjectedCloudsyncCredentials;

const DEVICE_NAME_HEADER = "x-anarlog-device-name";
const DEVICE_LIMIT_ERROR_CODE = "sync_device_limit_reached";
const DEVICE_LIMIT_TOAST_ID = "cloudsync-device-limit";

export type CloudsyncCredentialBlock =
  | "device_limit"
  | "identity_mismatch"
  | "not_entitled"
  | "reauth_required"
  | "setup_required"
  | "unavailable"
  | null;

let credentialBlock: CloudsyncCredentialBlock = null;
const credentialBlockListeners = new Set<() => void>();

function setCredentialBlock(next: CloudsyncCredentialBlock) {
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

function raceWithAbort<T>(operation: Promise<T>, signal: AbortSignal) {
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

function readStoredSettings() {
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

function readE2eeIdentity(userId: string) {
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

async function getDeviceIdentity() {
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

async function readCredentialErrorCode(
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

let generation = 0;
let exchangeController: AbortController | null = null;
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
let evictionRetryTimer: ReturnType<typeof setTimeout> | null = null;
let pluginOperation = Promise.resolve();
const pendingCloudsyncTeardowns = new Set<Promise<void>>();
const timedOutCloudsyncTeardowns = new Set<Promise<void>>();
let cleanupSuspendFailureVersion = 0;
let cleanupSuspendCompletedVersion = 0;
let signedOutGeneration: number | null = null;
let cleanupServiceGeneration: number | null = null;
let currentCloudsyncReactivation: {
  session: Session;
  generation: number;
  onAccountMismatch: CloudsyncAccountMismatchHandler | undefined;
} | null = null;

function beginTransition() {
  generation += 1;
  exchangeController?.abort();
  exchangeController = null;

  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
  if (evictionRetryTimer) {
    clearTimeout(evictionRetryTimer);
    evictionRetryTimer = null;
  }

  return generation;
}

function enqueuePluginOperation<T>(operation: () => Promise<T>) {
  const next = pluginOperation.then(operation, operation);
  pluginOperation = next.then(
    () => {},
    () => {},
  );
  return next;
}

function requireCleanupSuspend(serviceCurrent: boolean = true) {
  cleanupSuspendFailureVersion += 1;
  exchangeController?.abort();
  if (serviceCurrent) {
    serviceCurrentCloudsyncCleanup();
  }
}

function completeCleanupSuspend(failureVersion: number) {
  cleanupSuspendCompletedVersion = Math.max(
    cleanupSuspendCompletedVersion,
    failureVersion,
  );
}

function isCleanupSuspendRequired() {
  return cleanupSuspendFailureVersion > cleanupSuspendCompletedVersion;
}

function trackCloudsyncTeardown(
  teardown: Promise<void>,
  completesPriorCleanup: boolean = true,
) {
  const failureVersion = cleanupSuspendFailureVersion;
  const tracked = teardown
    .then(
      () => {
        if (completesPriorCleanup) {
          completeCleanupSuspend(failureVersion);
        }
      },
      (error) => {
        requireCleanupSuspend();
        throw error;
      },
    )
    .finally(() => {
      pendingCloudsyncTeardowns.delete(tracked);
      timedOutCloudsyncTeardowns.delete(tracked);
    });
  pendingCloudsyncTeardowns.add(tracked);
  return tracked;
}

async function settleCloudsyncOperationWithin<T>(
  operation: Promise<T>,
  timeoutMs: number = CLOUDSYNC_TEARDOWN_TIMEOUT_MS,
): Promise<{ status: "settled"; value: T } | { status: "timed_out" }> {
  let timeout: ReturnType<typeof setTimeout> | null = null;
  try {
    return await Promise.race([
      operation.then((value) => ({ status: "settled" as const, value })),
      new Promise<{ status: "timed_out" }>((resolve) => {
        timeout = setTimeout(() => resolve({ status: "timed_out" }), timeoutMs);
      }),
    ]);
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
  }
}

function getPendingCloudsyncTeardowns() {
  if (pendingCloudsyncTeardowns.size === 0) {
    return null;
  }
  return [...pendingCloudsyncTeardowns];
}

function stopWaitingForCloudsyncTeardowns(
  teardowns: Promise<void>[],
  activeGeneration: number,
) {
  if (activeGeneration !== generation) {
    return;
  }

  for (const teardown of teardowns) {
    if (pendingCloudsyncTeardowns.delete(teardown)) {
      timedOutCloudsyncTeardowns.add(teardown);
      void teardown.then(
        scheduleCurrentCloudsyncReactivation,
        scheduleCurrentCloudsyncReactivation,
      );
    }
  }
}

function shouldStopCloudsyncSessionEvictions(activeGeneration: number) {
  return (
    activeGeneration !== generation ||
    isCleanupSuspendRequired() ||
    timedOutCloudsyncTeardowns.size > 0
  );
}

async function flushCloudsyncSessionEvictions(
  activeGeneration: number,
): Promise<boolean> {
  const batchSize = 128;

  while (true) {
    if (shouldStopCloudsyncSessionEvictions(activeGeneration)) {
      return false;
    }

    let rows: { sessionId: string; workspaceId: string }[];
    try {
      rows = await execute(
        `
          SELECT
            eviction.session_id AS sessionId,
            eviction.workspace_id AS workspaceId
          FROM cloudsync_session_evictions AS eviction
          WHERE NOT EXISTS (
            SELECT 1
            FROM workspace_memberships AS membership
            WHERE membership.workspace_id = eviction.workspace_id
              AND membership.deleted_at IS NULL
          )
          AND NOT EXISTS (
            SELECT 1
            FROM sessions
            WHERE sessions.id = eviction.session_id
          )
          ORDER BY eviction.queued_at, eviction.session_id
          LIMIT ?
        `,
        [batchSize],
      );
    } catch (error) {
      console.warn("[cloudsync] session eviction queue unavailable", error);
      return true;
    }

    if (shouldStopCloudsyncSessionEvictions(activeGeneration)) {
      return false;
    }
    if (rows.length === 0) return false;

    let failed = false;
    for (const row of rows) {
      if (shouldStopCloudsyncSessionEvictions(activeGeneration)) {
        return false;
      }

      let deletionError = "";
      try {
        const result = await fsSyncCommands.deleteSessionFolder(row.sessionId);
        if (result.status === "error") {
          deletionError = String(result.error);
        }
      } catch (error) {
        deletionError = error instanceof Error ? error.message : String(error);
      }

      if (shouldStopCloudsyncSessionEvictions(activeGeneration)) {
        return false;
      }

      try {
        if (deletionError) {
          failed = true;
          await execute(
            `
              UPDATE cloudsync_session_evictions
              SET attempt_count = attempt_count + 1,
                  last_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                  last_error = ?
              WHERE session_id = ? AND workspace_id = ?
            `,
            [deletionError.slice(0, 512), row.sessionId, row.workspaceId],
          );
          continue;
        }

        await execute(
          `
            DELETE FROM cloudsync_session_evictions
            WHERE session_id = ? AND workspace_id = ?
              AND NOT EXISTS (
                SELECT 1
                FROM workspace_memberships
                WHERE workspace_id = ? AND deleted_at IS NULL
              )
              AND NOT EXISTS (
                SELECT 1 FROM sessions WHERE id = ?
              )
          `,
          [row.sessionId, row.workspaceId, row.workspaceId, row.sessionId],
        );
      } catch (error) {
        failed = true;
        console.warn(
          "[cloudsync] failed to update session eviction queue",
          error,
        );
      }
    }

    if (failed) return true;
    if (rows.length < batchSize) return false;
  }
}

function scheduleCloudsyncSessionEvictionRetry(activeGeneration: number) {
  if (evictionRetryTimer) {
    clearTimeout(evictionRetryTimer);
  }
  evictionRetryTimer = setTimeout(() => {
    evictionRetryTimer = null;
    if (shouldStopCloudsyncSessionEvictions(activeGeneration)) return;

    void enqueuePluginOperation(async () => {
      if (shouldStopCloudsyncSessionEvictions(activeGeneration)) return;
      const retry = await flushCloudsyncSessionEvictions(activeGeneration);
      if (retry && activeGeneration === generation) {
        scheduleCloudsyncSessionEvictionRetry(activeGeneration);
      }
    });
  }, EVICTION_RETRY_DELAY_MS);
}

export async function bindCloudsyncAccountForAuth(
  accountUserId: string,
): Promise<boolean> {
  const activeGeneration = beginTransition();
  signedOutGeneration = null;
  currentCloudsyncReactivation = null;
  const pendingTeardowns = getPendingCloudsyncTeardowns();
  let teardownTimedOut = false;

  if (pendingTeardowns) {
    let timeout: ReturnType<typeof setTimeout> | null = null;
    const result = await Promise.race([
      Promise.allSettled(pendingTeardowns).then(() => "settled" as const),
      new Promise<"timed_out">((resolve) => {
        timeout = setTimeout(
          () => resolve("timed_out"),
          CLOUDSYNC_TEARDOWN_TIMEOUT_MS,
        );
      }),
    ]);
    if (timeout) {
      clearTimeout(timeout);
    }
    if (activeGeneration !== generation) {
      return true;
    }
    if (result === "timed_out") {
      teardownTimedOut = true;
      stopWaitingForCloudsyncTeardowns(pendingTeardowns, activeGeneration);
    }
  }

  const preemptPluginQueue =
    teardownTimedOut ||
    timedOutCloudsyncTeardowns.size > 0 ||
    isCleanupSuspendRequired();
  if (preemptPluginQueue) {
    const cleanup = await settleCloudsyncOperationWithin(
      suspendCloudsyncPreemptivelyForGeneration(activeGeneration),
    );
    if (activeGeneration !== generation) {
      return true;
    }
    if (cleanup.status === "timed_out" || !cleanup.value) {
      throw new Error("cloudsync cleanup unavailable");
    }
  }

  let claimed: boolean;
  if (preemptPluginQueue) {
    claimed = await bindCloudsyncAccount(accountUserId);
  } else {
    const queuedBinding = await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) {
        return { status: "stale" as const };
      }
      if (isCleanupSuspendRequired()) {
        return { status: "cleanup_required" as const };
      }
      return {
        status: "bound" as const,
        claimed: await bindCloudsyncAccount(accountUserId),
      };
    });
    if (activeGeneration !== generation || queuedBinding.status === "stale") {
      return true;
    }
    if (queuedBinding.status === "cleanup_required") {
      const cleanup = await settleCloudsyncOperationWithin(
        suspendCloudsyncPreemptivelyForGeneration(activeGeneration),
      );
      if (activeGeneration !== generation) {
        return true;
      }
      if (cleanup.status === "timed_out" || !cleanup.value) {
        throw new Error("cloudsync cleanup unavailable");
      }
      claimed = await bindCloudsyncAccount(accountUserId);
    } else {
      claimed = queuedBinding.claimed;
    }
  }
  if (activeGeneration !== generation) {
    return true;
  }

  if (isCleanupSuspendRequired()) {
    const cleanup = await settleCloudsyncOperationWithin(
      suspendCloudsyncPreemptivelyForGeneration(activeGeneration),
    );
    if (activeGeneration !== generation) {
      return true;
    }
    if (cleanup.status === "timed_out" || !cleanup.value) {
      throw new Error("cloudsync cleanup unavailable");
    }
  }

  return claimed;
}

async function suspendCloudsyncNow() {
  const failureVersion = cleanupSuspendFailureVersion;
  try {
    await suspendCloudsync();
  } catch (error) {
    requireCleanupSuspend();
    throw error;
  }
  completeCleanupSuspend(failureVersion);
  stopCloudsyncInitialSyncProgress();
}

async function suspendCloudsyncForAuthCleanupNow(
  serviceCurrentOnFailure: boolean = true,
) {
  const failureVersion = cleanupSuspendFailureVersion;
  try {
    await suspendCloudsync();
  } catch (error) {
    requireCleanupSuspend(serviceCurrentOnFailure);
    throw error;
  }
  completeCleanupSuspend(failureVersion);
  stopCloudsyncInitialSyncProgress();
}

async function suspendCloudsyncForGeneration(activeGeneration: number) {
  if (activeGeneration !== generation) {
    return false;
  }

  try {
    await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) {
        return;
      }
      await suspendCloudsyncNow();
    });
  } catch {
    if (activeGeneration === generation) {
      console.warn("[cloudsync] local sync suspension failed");
    }
    return false;
  }

  if (activeGeneration !== generation) {
    if (isCleanupSuspendRequired()) {
      scheduleCurrentCloudsyncReactivation();
    }
    return false;
  }

  return !isCleanupSuspendRequired();
}

async function suspendCloudsyncPreemptivelyForGeneration(
  activeGeneration: number,
  serviceCurrentOnFailure: boolean = true,
) {
  if (activeGeneration !== generation) {
    return false;
  }

  try {
    await suspendCloudsyncForAuthCleanupNow(serviceCurrentOnFailure);
  } catch {
    if (activeGeneration === generation) {
      console.warn("[cloudsync] local sync suspension failed");
    }
  }

  if (activeGeneration !== generation) {
    if (isCleanupSuspendRequired()) {
      serviceCurrentCloudsyncCleanup();
    } else {
      scheduleCurrentCloudsyncReactivation();
    }
    return false;
  }

  return !isCleanupSuspendRequired();
}

function serviceCurrentCloudsyncCleanup() {
  if (!isCleanupSuspendRequired()) {
    return;
  }

  if (signedOutGeneration !== generation) {
    scheduleCurrentCloudsyncReactivation();
    return;
  }

  const activeGeneration = generation;
  if (cleanupServiceGeneration === activeGeneration) {
    return;
  }
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }

  cleanupServiceGeneration = activeGeneration;
  void suspendCloudsyncPreemptivelyForGeneration(activeGeneration).then(
    (suspended) => {
      if (cleanupServiceGeneration === activeGeneration) {
        cleanupServiceGeneration = null;
      }
      if (
        activeGeneration !== generation ||
        signedOutGeneration !== activeGeneration ||
        (suspended && !isCleanupSuspendRequired())
      ) {
        return;
      }

      refreshTimer = setTimeout(() => {
        refreshTimer = null;
        serviceCurrentCloudsyncCleanup();
      }, RETRY_DELAY_MS);
    },
  );
}

async function suspendCloudsyncAfterCredentialRejection(
  activeGeneration: number,
) {
  const cleanup = await settleCloudsyncOperationWithin(
    suspendCloudsyncPreemptivelyForGeneration(activeGeneration, false),
  );
  if (cleanup.status === "timed_out" && activeGeneration === generation) {
    requireCleanupSuspend(false);
  }
  if (cleanup.status === "timed_out" || !cleanup.value) {
    if (activeGeneration === generation) {
      refreshTimer = setTimeout(() => {
        refreshTimer = null;
        if (activeGeneration !== generation) {
          return;
        }
        if (!isCleanupSuspendRequired()) {
          return;
        }

        void suspendCloudsyncAfterCredentialRejection(activeGeneration);
      }, RETRY_DELAY_MS);
    }
  }
}

function isCredentials(value: unknown): value is CloudsyncCredentials {
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

function hasWorkspaceProjection(
  credentials: CloudsyncCredentials,
): credentials is ProjectedCloudsyncCredentials {
  return credentials.accountUserId !== undefined;
}

function scheduleExchange(
  session: Session,
  activeGeneration: number,
  delayMs: number,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
) {
  if (activeGeneration !== generation) {
    return;
  }

  if (refreshTimer) {
    clearTimeout(refreshTimer);
  }
  refreshTimer = setTimeout(() => {
    refreshTimer = null;
    if (activeGeneration !== generation) {
      return;
    }

    void activateCloudsync(session, false, onAccountMismatch).then(
      async (result) => {
        if (result !== "account_mismatch" || !onAccountMismatch) {
          return;
        }

        try {
          await onAccountMismatch();
        } catch {
          console.warn("[cloudsync] account mismatch rejection failed");
        }
      },
    );
  }, delayMs);
}

function scheduleCurrentCloudsyncReactivation() {
  const reactivation = currentCloudsyncReactivation;
  if (!reactivation || reactivation.generation !== generation) {
    return;
  }
  scheduleExchange(
    reactivation.session,
    reactivation.generation,
    MIN_REFRESH_DELAY_MS,
    reactivation.onAccountMismatch,
  );
}

function scheduleActivityStatusRetry(
  session: Session,
  activeGeneration: number,
  suspendBeforeExchange: boolean,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
) {
  if (activeGeneration !== generation) {
    return;
  }

  refreshTimer = setTimeout(() => {
    refreshTimer = null;
    if (activeGeneration !== generation) {
      return;
    }

    void enqueuePluginOperation(async () => {
      if (activeGeneration !== generation || isCleanupSuspendRequired()) {
        return Promise.resolve(null);
      }
      return getCloudsyncStatus();
    })
      .then((status) => {
        if (activeGeneration !== generation) {
          return;
        }
        if (isCleanupSuspendRequired()) {
          scheduleExchange(
            session,
            activeGeneration,
            MIN_REFRESH_DELAY_MS,
            onAccountMismatch,
          );
          return;
        }
        if (!status) return;
        if (status.activity_paused) {
          scheduleActivityStatusRetry(
            session,
            activeGeneration,
            suspendBeforeExchange,
            onAccountMismatch,
          );
          return;
        }

        void activateCloudsync(
          session,
          suspendBeforeExchange,
          onAccountMismatch,
        ).then(async (result) => {
          if (result !== "account_mismatch" || !onAccountMismatch) {
            return;
          }

          try {
            await onAccountMismatch();
          } catch {
            console.warn("[cloudsync] account mismatch rejection failed");
          }
        });
      })
      .catch(() => {
        if (activeGeneration !== generation) {
          return;
        }
        if (isCleanupSuspendRequired()) {
          scheduleExchange(
            session,
            activeGeneration,
            MIN_REFRESH_DELAY_MS,
            onAccountMismatch,
          );
          return;
        }
        console.warn(
          "[cloudsync] activity status unavailable; retrying without credentials",
        );
        scheduleActivityStatusRetry(
          session,
          activeGeneration,
          suspendBeforeExchange,
          onAccountMismatch,
        );
      });
  }, ACTIVITY_RETRY_DELAY_MS);
}

async function activateCloudsync(
  session: Session,
  suspendBeforeExchange: boolean,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<CloudsyncAuthChangeResult> {
  const activeGeneration = beginTransition();
  signedOutGeneration = null;
  currentCloudsyncReactivation = {
    session,
    generation: activeGeneration,
    onAccountMismatch,
  };
  const scheduleReactivation = () => {
    if (activeGeneration === generation) {
      scheduleExchange(
        session,
        activeGeneration,
        MIN_REFRESH_DELAY_MS,
        onAccountMismatch,
      );
    }
  };

  const pendingTeardowns = getPendingCloudsyncTeardowns();
  if (pendingTeardowns) {
    let finishedWaiting = false;
    const timeout = setTimeout(() => {
      if (finishedWaiting) {
        return;
      }
      if (activeGeneration !== generation) {
        return;
      }
      finishedWaiting = true;
      stopWaitingForCloudsyncTeardowns(pendingTeardowns, activeGeneration);
      scheduleReactivation();
    }, CLOUDSYNC_TEARDOWN_TIMEOUT_MS);
    void Promise.allSettled(pendingTeardowns).then(() => {
      if (finishedWaiting) {
        return;
      }
      finishedWaiting = true;
      clearTimeout(timeout);
      scheduleReactivation();
    });
    return "ok";
  }

  let suspendedBeforeCredentialExchange = false;
  if (isCleanupSuspendRequired()) {
    if (!(await suspendCloudsyncPreemptivelyForGeneration(activeGeneration))) {
      if (activeGeneration === generation) {
        scheduleExchange(
          session,
          activeGeneration,
          RETRY_DELAY_MS,
          onAccountMismatch,
        );
      }
      return "ok";
    }
    if (isCleanupSuspendRequired()) {
      scheduleReactivation();
      return "ok";
    }
    suspendedBeforeCredentialExchange = true;
  }

  let enabled: boolean;
  try {
    const settings = await settleCloudsyncOperationWithin(
      readStoredSettings(),
      EXCHANGE_TIMEOUT_MS,
    );
    if (settings.status === "timed_out") {
      throw new Error("sync preference read timed out");
    }
    enabled = resolveConfigValue("cloud_sync_enabled", settings.value);
  } catch {
    console.warn(
      "[cloudsync] sync preference is unavailable; sync remains disabled",
    );
    if (activeGeneration === generation) {
      setCredentialBlock("unavailable");
    }
    if (!suspendedBeforeCredentialExchange) {
      await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    }
    if (activeGeneration === generation) {
      scheduleExchange(
        session,
        activeGeneration,
        RETRY_DELAY_MS,
        onAccountMismatch,
      );
    }
    return "ok";
  }

  if (activeGeneration !== generation) {
    return "ok";
  }

  if (!enabled) {
    setCredentialBlock(null);
    if (!suspendedBeforeCredentialExchange) {
      await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    }
    return "ok";
  }

  let status;
  try {
    status = await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation || isCleanupSuspendRequired()) {
        return null;
      }
      return getCloudsyncStatus();
    });
  } catch {
    if (activeGeneration === generation && isCleanupSuspendRequired()) {
      const suspended =
        await suspendCloudsyncPreemptivelyForGeneration(activeGeneration);
      if (activeGeneration === generation) {
        if (suspended) {
          scheduleReactivation();
        } else {
          scheduleExchange(
            session,
            activeGeneration,
            RETRY_DELAY_MS,
            onAccountMismatch,
          );
        }
      }
      return "ok";
    }
    if (activeGeneration === generation) {
      console.warn("[cloudsync] local sync status unavailable; retrying");
      scheduleExchange(
        session,
        activeGeneration,
        RETRY_DELAY_MS,
        onAccountMismatch,
      );
    }
    return "ok";
  }

  if (activeGeneration !== generation) {
    return "ok";
  }
  if (!status) {
    scheduleReactivation();
    return "ok";
  }

  if (isCleanupSuspendRequired()) {
    if (!(await suspendCloudsyncPreemptivelyForGeneration(activeGeneration))) {
      if (activeGeneration === generation) {
        scheduleExchange(
          session,
          activeGeneration,
          RETRY_DELAY_MS,
          onAccountMismatch,
        );
      }
      return "ok";
    }
    if (isCleanupSuspendRequired()) {
      scheduleReactivation();
      return "ok";
    }
    suspendedBeforeCredentialExchange = true;
  }

  if (!status.cloudsync_enabled) {
    setCredentialBlock("unavailable");
    console.warn(
      "[cloudsync] native sync is unavailable; sync remains disabled",
    );
    return "ok";
  }

  if (status.activity_paused) {
    scheduleActivityStatusRetry(
      session,
      activeGeneration,
      suspendBeforeExchange && !suspendedBeforeCredentialExchange,
      onAccountMismatch,
    );
    return "ok";
  }

  let encryptionKeyId: string;
  try {
    const identityRead = await settleCloudsyncOperationWithin(
      readE2eeIdentity(session.user.id),
      EXCHANGE_TIMEOUT_MS,
    );
    if (identityRead.status === "timed_out") {
      throw new Error("E2EE identity read timed out");
    }
    const identity = identityRead.value;
    if (activeGeneration !== generation) {
      return "ok";
    }
    if (isCleanupSuspendRequired()) {
      if (
        !(await suspendCloudsyncPreemptivelyForGeneration(activeGeneration))
      ) {
        if (activeGeneration === generation) {
          scheduleExchange(
            session,
            activeGeneration,
            RETRY_DELAY_MS,
            onAccountMismatch,
          );
        }
        return "ok";
      }
      if (isCleanupSuspendRequired()) {
        scheduleReactivation();
        return "ok";
      }
      suspendedBeforeCredentialExchange = true;
    }
    if (
      !identity.configured ||
      !identity.keyId ||
      !/^[A-Za-z0-9_-]{22}$/.test(identity.keyId)
    ) {
      setCredentialBlock("setup_required");
      await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      console.warn(
        "[cloudsync] E2EE recovery key setup is required; sync remains disabled",
      );
      return "ok";
    }
    encryptionKeyId = identity.keyId;
  } catch {
    if (isCleanupSuspendRequired()) {
      await suspendCloudsyncPreemptivelyForGeneration(activeGeneration);
      scheduleReactivation();
      return "ok";
    }
    if (activeGeneration === generation) {
      setCredentialBlock("unavailable");
    }
    await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    console.warn(
      "[cloudsync] E2EE recovery key is unavailable; sync remains disabled",
    );
    if (activeGeneration === generation) {
      scheduleExchange(
        session,
        activeGeneration,
        RETRY_DELAY_MS,
        onAccountMismatch,
      );
    }
    return "ok";
  }

  if (isCleanupSuspendRequired()) {
    if (!(await suspendCloudsyncPreemptivelyForGeneration(activeGeneration))) {
      if (activeGeneration === generation) {
        scheduleExchange(
          session,
          activeGeneration,
          RETRY_DELAY_MS,
          onAccountMismatch,
        );
      }
      return "ok";
    }
    if (isCleanupSuspendRequired()) {
      scheduleReactivation();
      return "ok";
    }
    suspendedBeforeCredentialExchange = true;
  }

  const shouldSuspendBeforeExchange =
    suspendBeforeExchange &&
    !suspendedBeforeCredentialExchange &&
    timedOutCloudsyncTeardowns.size === 0;
  if (
    shouldSuspendBeforeExchange &&
    !(await suspendCloudsyncForGeneration(activeGeneration))
  ) {
    if (activeGeneration === generation) {
      scheduleExchange(
        session,
        activeGeneration,
        RETRY_DELAY_MS,
        onAccountMismatch,
      );
    }
    return "ok";
  }

  if (isCleanupSuspendRequired()) {
    scheduleReactivation();
    return "ok";
  }

  const controller = new AbortController();
  exchangeController = controller;
  let exchangeTimedOut = false;
  const exchangeTimeout = setTimeout(() => {
    exchangeTimedOut = true;
    controller.abort();
  }, EXCHANGE_TIMEOUT_MS);

  let response: Response | null = null;
  let credentials: unknown;
  let credentialErrorCode: string | null = null;
  try {
    const device = await raceWithAbort(getDeviceIdentity(), controller.signal);
    if (isCleanupSuspendRequired()) {
      scheduleReactivation();
      return "ok";
    }
    const headers: Record<string, string> = {
      Authorization: `Bearer ${session.access_token}`,
      "X-Anarlog-E2EE-Key-Id": encryptionKeyId,
    };
    if (device.fingerprint) {
      headers[DEVICE_FINGERPRINT_HEADER] = device.fingerprint;
    }
    if (device.name) {
      headers[DEVICE_NAME_HEADER] = device.name;
    }

    response = await raceWithAbort(
      fetch(new URL("/sync/token", env.VITE_API_URL), {
        method: "POST",
        headers,
        signal: controller.signal,
      }),
      controller.signal,
    );

    if (response.ok) {
      credentials = await raceWithAbort(response.json(), controller.signal);
    } else if (response.status === 403) {
      try {
        credentialErrorCode = await raceWithAbort(
          readCredentialErrorCode(response),
          controller.signal,
        );
      } catch {
        if (activeGeneration !== generation) {
          return "ok";
        }
        credentialErrorCode = null;
      }
    }
  } catch {
    if (
      activeGeneration !== generation ||
      (controller.signal.aborted && !exchangeTimedOut)
    ) {
      return "ok";
    }

    console.warn(
      response === null
        ? "[cloudsync] credential exchange unavailable; retrying"
        : "[cloudsync] credential exchange returned an invalid response",
    );
    scheduleExchange(
      session,
      activeGeneration,
      RETRY_DELAY_MS,
      onAccountMismatch,
    );
    return "ok";
  } finally {
    clearTimeout(exchangeTimeout);
    if (exchangeController === controller) {
      exchangeController = null;
    }
  }

  if (activeGeneration !== generation) {
    return "ok";
  }

  if (isCleanupSuspendRequired()) {
    scheduleReactivation();
    return "ok";
  }

  if (!response.ok) {
    if (response.status === 404 || response.status === 501) {
      setCredentialBlock("unavailable");
      if (!shouldSuspendBeforeExchange) {
        await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      }
      console.warn("[cloudsync] credential exchange is not configured");
      return "ok";
    }

    if (response.status === 403) {
      if (activeGeneration !== generation) {
        return "ok";
      }
      setCredentialBlock(
        credentialErrorCode === DEVICE_LIMIT_ERROR_CODE
          ? "device_limit"
          : "not_entitled",
      );
      if (!shouldSuspendBeforeExchange) {
        await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      }
      if (credentialErrorCode === DEVICE_LIMIT_ERROR_CODE) {
        sonnerToast.error(
          t`Cloud sync is limited to 5 devices. Remove another device to sync here.`,
          { id: DEVICE_LIMIT_TOAST_ID },
        );
        console.warn(
          "[cloudsync] device limit reached; sync remains disabled on this device",
        );
        return "ok";
      }
      console.warn(
        "[cloudsync] Anarlog Pro is required; sync remains disabled",
      );
      return "ok";
    }

    if (response.status === 401) {
      setCredentialBlock("reauth_required");
      if (!shouldSuspendBeforeExchange) {
        await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      }
      console.warn("[cloudsync] credential exchange requires a fresh session");
      return "ok";
    }

    console.warn("[cloudsync] credential exchange unavailable; retrying");
    scheduleExchange(
      session,
      activeGeneration,
      RETRY_DELAY_MS,
      onAccountMismatch,
    );
    return "ok";
  }

  if (activeGeneration !== generation) {
    return "ok";
  }

  if (!isCredentials(credentials)) {
    console.warn(
      "[cloudsync] credential exchange returned an invalid response",
    );
    scheduleExchange(
      session,
      activeGeneration,
      RETRY_DELAY_MS,
      onAccountMismatch,
    );
    return "ok";
  }

  if (credentials.encryptionKeyId !== encryptionKeyId) {
    setCredentialBlock("identity_mismatch");
    await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    console.warn(
      "[cloudsync] credential exchange returned a different E2EE key identity",
    );
    return "ok";
  }

  const accountUserId = hasWorkspaceProjection(credentials)
    ? credentials.accountUserId
    : credentials.workspaceId;
  if (accountUserId !== session.user.id) {
    setCredentialBlock("identity_mismatch");
    if (!shouldSuspendBeforeExchange) {
      await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    }
    console.warn(
      "[cloudsync] credential exchange returned an invalid workspace",
    );
    return "ok";
  }

  const expiresAtMs = Date.parse(credentials.expiresAt);
  if (expiresAtMs <= Date.now()) {
    console.warn("[cloudsync] credential exchange returned an expired token");
    scheduleExchange(
      session,
      activeGeneration,
      RETRY_DELAY_MS,
      onAccountMismatch,
    );
    return "ok";
  }

  setCredentialBlock(null);

  try {
    const configured = await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) {
        return "configured" as const;
      }
      if (isCleanupSuspendRequired()) {
        return "cleanup_required" as const;
      }

      const configuration = hasWorkspaceProjection(credentials)
        ? await configureCloudsyncToken(
            credentials.databaseId,
            credentials.token,
            accountUserId,
            {
              endpoint: new URL(
                `/sync/e2ee/witness/${credentials.personalWorkspaceId}`,
                env.VITE_API_URL,
              ).toString(),
              accessToken: session.access_token,
            },
            {
              accountUserId: credentials.accountUserId,
              personalWorkspaceId: credentials.personalWorkspaceId,
              workspaces: credentials.workspaces,
            },
          )
        : await configureCloudsyncToken(
            credentials.databaseId,
            credentials.token,
            accountUserId,
            {
              endpoint: new URL(
                `/sync/e2ee/witness/${credentials.workspaceId}`,
                env.VITE_API_URL,
              ).toString(),
              accessToken: session.access_token,
            },
          );

      let cleanupRequired = isCleanupSuspendRequired();
      if (activeGeneration !== generation || cleanupRequired) {
        if (configuration === "configured") {
          await suspendCloudsyncNow();
        }
        return cleanupRequired ? ("cleanup_required" as const) : configuration;
      }

      if (configuration === "configured" && activeGeneration === generation) {
        const retryEvictions =
          await flushCloudsyncSessionEvictions(activeGeneration);
        if (retryEvictions) {
          scheduleCloudsyncSessionEvictionRetry(activeGeneration);
        }
      }

      cleanupRequired = isCleanupSuspendRequired();
      if (activeGeneration !== generation || cleanupRequired) {
        if (configuration === "configured") {
          await suspendCloudsyncNow();
        }
        return cleanupRequired ? ("cleanup_required" as const) : configuration;
      }

      return configuration;
    });

    if (activeGeneration !== generation) {
      return "ok";
    }

    if (configured === "cleanup_required" || isCleanupSuspendRequired()) {
      if (isCleanupSuspendRequired()) {
        await suspendCloudsyncPreemptivelyForGeneration(activeGeneration);
      }
      scheduleReactivation();
      return "ok";
    }

    if (configured === "account_mismatch") {
      console.warn("[cloudsync] local database belongs to another account");
      return "account_mismatch";
    }

    startCloudsyncInitialSyncProgress(session.user.id);
  } catch (error) {
    if (activeGeneration !== generation) {
      return "ok";
    }

    if (isCleanupSuspendRequired()) {
      await suspendCloudsyncPreemptivelyForGeneration(activeGeneration);
      scheduleReactivation();
      return "ok";
    }

    if (isCloudsyncActivityDeferredError(error)) {
      scheduleActivityStatusRetry(
        session,
        activeGeneration,
        shouldSuspendBeforeExchange,
        onAccountMismatch,
      );
      return "ok";
    }

    await suspendCloudsyncPreemptivelyForGeneration(activeGeneration);

    console.warn(
      "[cloudsync] local sync configuration failed; retrying",
      error,
    );
    scheduleExchange(
      session,
      activeGeneration,
      RETRY_DELAY_MS,
      onAccountMismatch,
    );
    return "ok";
  }

  const timeUntilExpiryMs = expiresAtMs - Date.now();
  const refreshLeadMs = Math.min(
    REFRESH_LEAD_MS,
    Math.max(MIN_REFRESH_DELAY_MS, timeUntilExpiryMs / 5),
  );
  scheduleExchange(
    session,
    activeGeneration,
    Math.max(MIN_REFRESH_DELAY_MS, timeUntilExpiryMs - refreshLeadMs),
    onAccountMismatch,
  );
  return "ok";
}

async function suspendCloudsyncSession(): Promise<void> {
  const activeGeneration = beginTransition();
  signedOutGeneration = activeGeneration;
  currentCloudsyncReactivation = null;
  stopCloudsyncInitialSyncProgress();
  setCredentialBlock(null);
  const suspension = trackCloudsyncTeardown(suspendCloudsyncAfterAuthLoss());
  let timeout: ReturnType<typeof setTimeout> | null = null;

  try {
    const result = await Promise.race([
      suspension.then(() => "suspended" as const),
      new Promise<"timed_out">((resolve) => {
        timeout = setTimeout(
          () => resolve("timed_out"),
          CLOUDSYNC_TEARDOWN_TIMEOUT_MS,
        );
      }),
    ]);

    if (result === "timed_out") {
      console.warn(
        "[cloudsync] auth-loss suspension is finishing in background",
      );
      requireCleanupSuspend();
      void suspension.catch((error) => {
        console.warn(
          "[cloudsync] background auth-loss suspension failed",
          error,
        );
      });
    } else if (activeGeneration === generation && isCleanupSuspendRequired()) {
      serviceCurrentCloudsyncCleanup();
    }
  } catch {
    console.warn("[cloudsync] local sync suspension failed");
    serviceCurrentCloudsyncCleanup();
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
  }
}

export async function prepareCloudsyncSignOut(
  session: Session | null | undefined,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<void> {
  const activeGeneration = beginTransition();
  signedOutGeneration = session ? null : activeGeneration;
  currentCloudsyncReactivation = null;
  stopCloudsyncInitialSyncProgress();
  requireCleanupSuspend(false);
  const suspension = trackCloudsyncTeardown(
    suspendCloudsyncForSignOut(),
    false,
  );
  let timeout: ReturnType<typeof setTimeout> | null = null;

  try {
    const result = await Promise.race([
      suspension.then(() => "suspended" as const),
      new Promise<"timed_out">((resolve) => {
        timeout = setTimeout(
          () => resolve("timed_out"),
          CLOUDSYNC_TEARDOWN_TIMEOUT_MS,
        );
      }),
    ]);

    if (result === "timed_out") {
      console.warn(
        "[cloudsync] sign-out suspension is finishing in background",
      );
      void suspension.catch((error) => {
        console.warn(
          "[cloudsync] background sign-out suspension failed",
          error,
        );
      });
    }
  } catch (error) {
    if (session) {
      scheduleExchange(
        session,
        activeGeneration,
        MIN_REFRESH_DELAY_MS,
        onAccountMismatch,
      );
    }
    throw error;
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
    if (!session) {
      serviceCurrentCloudsyncCleanup();
    }
  }
}

export async function handleCloudsyncAuthChange(
  _event: AuthChangeEvent,
  session: Session | null,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<CloudsyncAuthChangeResult> {
  if (!session) {
    await suspendCloudsyncSession();
    return "ok";
  }

  return activateCloudsync(session, true, onAccountMismatch);
}

export async function applyCloudsyncPreference(
  session: Session | null | undefined,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<CloudsyncAuthChangeResult> {
  if (!session) {
    await suspendCloudsyncSession();
    return "ok";
  }

  return activateCloudsync(session, true, onAccountMismatch);
}

export async function refreshCloudsyncForSession(
  session: Session,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<CloudsyncAuthChangeResult> {
  return activateCloudsync(session, false, onAccountMismatch);
}
