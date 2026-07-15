import type { AuthChangeEvent, Session } from "@supabase/supabase-js";

import {
  bindCloudsyncAccount,
  configureCloudsyncToken,
  getCloudsyncStatus,
  suspendCloudsync,
} from "@hypr/plugin-db";

import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

import { env } from "~/env";

const REFRESH_LEAD_MS = 2 * 60 * 1000;
const RETRY_DELAY_MS = 60 * 1000;
const MIN_REFRESH_DELAY_MS = 1000;
const EXCHANGE_TIMEOUT_MS = 10 * 1000;

export type CloudsyncAuthChangeResult = "ok" | "account_mismatch";

type CloudsyncAccountMismatchHandler = () => Promise<void>;

type CloudsyncCredentials = {
  databaseId: string;
  token: string;
  expiresAt: string;
  workspaceId: string;
};

let generation = 0;
let exchangeController: AbortController | null = null;
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
let pluginOperation = Promise.resolve();

function beginTransition() {
  generation += 1;
  exchangeController?.abort();
  exchangeController = null;

  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
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
  return (
    typeof candidate.databaseId === "string" &&
    candidate.databaseId.length > 0 &&
    typeof candidate.token === "string" &&
    candidate.token.length > 0 &&
    typeof candidate.expiresAt === "string" &&
    Number.isFinite(Date.parse(candidate.expiresAt)) &&
    typeof candidate.workspaceId === "string" &&
    candidate.workspaceId.length > 0
  );
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

  if (credentials.workspaceId !== session.user.id) {
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

      const configuration = await configureCloudsyncToken(
        credentials.databaseId,
        credentials.token,
        credentials.workspaceId,
      );

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

export async function refreshCloudsyncForSession(
  session: Session,
  onAccountMismatch?: CloudsyncAccountMismatchHandler,
): Promise<CloudsyncAuthChangeResult> {
  return activateCloudsync(session, false, onAccountMismatch);
}
