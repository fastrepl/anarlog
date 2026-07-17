import type { AuthChangeEvent, Session } from "@supabase/supabase-js";

import type { CloudsyncWorkspaceProjection } from "@hypr/plugin-db";
import {
  bindCloudsyncAccount,
  configureCloudsyncToken,
  execute,
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  suspendCloudsync,
} from "@hypr/plugin-db";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

import { env } from "~/env";
import { getStoredSettingValues } from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";

const REFRESH_LEAD_MS = 2 * 60 * 1000;
const RETRY_DELAY_MS = 60 * 1000;
const MIN_REFRESH_DELAY_MS = 1000;
const EXCHANGE_TIMEOUT_MS = 25 * 1000;
const EVICTION_RETRY_DELAY_MS = 30 * 1000;

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

let generation = 0;
let exchangeController: AbortController | null = null;
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
let evictionRetryTimer: ReturnType<typeof setTimeout> | null = null;
let pluginOperation = Promise.resolve();

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

async function flushCloudsyncSessionEvictions(): Promise<boolean> {
  const batchSize = 128;

  while (true) {
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

    if (rows.length === 0) return false;

    let failed = false;
    for (const row of rows) {
      let deletionError = "";
      try {
        const result = await fsSyncCommands.deleteSessionFolder(row.sessionId);
        if (result.status === "error") {
          deletionError = String(result.error);
        }
      } catch (error) {
        deletionError = error instanceof Error ? error.message : String(error);
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
    if (activeGeneration !== generation) return;

    void enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) return;
      const retry = await flushCloudsyncSessionEvictions();
      if (retry && activeGeneration === generation) {
        scheduleCloudsyncSessionEvictionRetry(activeGeneration);
      }
    });
  }, EVICTION_RETRY_DELAY_MS);
}

export async function bindCloudsyncAccountForAuth(
  accountUserId: string,
): Promise<boolean> {
  return enqueuePluginOperation(() => bindCloudsyncAccount(accountUserId));
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
      await suspendCloudsync();
      stopCloudsyncInitialSyncProgress();
    });
  } catch {
    if (activeGeneration === generation) {
      console.warn("[cloudsync] local sync suspension failed");
    }
    return false;
  }

  return activeGeneration === generation;
}

async function suspendCloudsyncAfterCredentialRejection(
  activeGeneration: number,
) {
  if (!(await suspendCloudsyncForGeneration(activeGeneration))) {
    if (activeGeneration === generation) {
      refreshTimer = setTimeout(() => {
        if (activeGeneration !== generation) {
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

  refreshTimer = setTimeout(() => {
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

async function activateCloudsync(
  session: Session,
  suspendBeforeExchange: boolean,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<CloudsyncAuthChangeResult> {
  const activeGeneration = beginTransition();
  let enabled: boolean;
  try {
    enabled = resolveConfigValue(
      "cloud_sync_enabled",
      await getStoredSettingValues(),
    );
  } catch {
    console.warn(
      "[cloudsync] sync preference is unavailable; sync remains disabled",
    );
    await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    return "ok";
  }

  if (!enabled) {
    await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    return "ok";
  }

  let encryptionKeyId: string;
  try {
    const identity = await enqueuePluginOperation(() =>
      getE2eeIdentityStatus(session.user.id),
    );
    if (
      !identity.configured ||
      !identity.keyId ||
      !/^[A-Za-z0-9_-]{22}$/.test(identity.keyId)
    ) {
      await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      console.warn(
        "[cloudsync] E2EE recovery key setup is required; sync remains disabled",
      );
      return "ok";
    }
    encryptionKeyId = identity.keyId;
  } catch {
    await suspendCloudsyncAfterCredentialRejection(activeGeneration);
    console.warn(
      "[cloudsync] E2EE recovery key is unavailable; sync remains disabled",
    );
    return "ok";
  }

  if (
    suspendBeforeExchange &&
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

  let status;
  try {
    status = await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) {
        return null;
      }
      return getCloudsyncStatus();
    });
  } catch {
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

  if (activeGeneration !== generation || !status) {
    return "ok";
  }

  if (!status.cloudsync_enabled) {
    console.warn(
      "[cloudsync] native sync is unavailable; sync remains disabled",
    );
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
  try {
    response = await fetch(new URL("/sync/token", env.VITE_API_URL), {
      method: "POST",
      headers: {
        Authorization: `Bearer ${session.access_token}`,
        "X-Anarlog-E2EE-Key-Id": encryptionKeyId,
      },
      signal: controller.signal,
    });

    if (response.ok) {
      credentials = await response.json();
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

  if (!response.ok) {
    if (response.status === 404 || response.status === 501) {
      if (!suspendBeforeExchange) {
        await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      }
      console.warn("[cloudsync] credential exchange is not configured");
      return "ok";
    }

    if (response.status === 403) {
      if (!suspendBeforeExchange) {
        await suspendCloudsyncAfterCredentialRejection(activeGeneration);
      }
      console.warn(
        "[cloudsync] Anarlog Pro is required; sync remains disabled",
      );
      return "ok";
    }

    if (response.status === 401) {
      if (!suspendBeforeExchange) {
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
    if (!suspendBeforeExchange) {
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

  try {
    const configured = await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) {
        return "configured" as const;
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

      if (configuration === "configured" && activeGeneration === generation) {
        const retryEvictions = await flushCloudsyncSessionEvictions();
        if (retryEvictions) {
          scheduleCloudsyncSessionEvictionRetry(activeGeneration);
        }
      }

      if (activeGeneration !== generation) {
        if (configuration === "configured") {
          await suspendCloudsync();
        }
      }

      return configuration;
    });

    if (activeGeneration !== generation) {
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

    try {
      await enqueuePluginOperation(suspendCloudsync);
    } catch {
      console.warn("[cloudsync] local sync suspension failed");
    }

    console.warn("[cloudsync] local sync configuration failed; retrying");
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
  beginTransition();
  stopCloudsyncInitialSyncProgress();

  try {
    await enqueuePluginOperation(suspendCloudsync);
  } catch {
    console.warn("[cloudsync] local sync suspension failed");
  }
}

export async function prepareCloudsyncSignOut(
  session: Session | null | undefined,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<void> {
  const activeGeneration = beginTransition();
  stopCloudsyncInitialSyncProgress();

  try {
    await enqueuePluginOperation(suspendCloudsync);
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
