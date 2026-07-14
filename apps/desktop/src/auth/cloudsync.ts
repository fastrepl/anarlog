import type { AuthChangeEvent, Session } from "@supabase/supabase-js";

import {
  configureCloudsyncToken,
  logoutCloudsync,
  suspendCloudsync,
} from "@hypr/plugin-db";

import { env } from "~/env";

const REFRESH_LEAD_MS = 2 * 60 * 1000;
const RETRY_DELAY_MS = 60 * 1000;
const MIN_REFRESH_DELAY_MS = 1000;

export type CloudsyncAuthChangeResult = "ok" | "account_mismatch";

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
    });
  } catch {
    if (activeGeneration === generation) {
      console.warn("[cloudsync] local sync suspension failed");
    }
    return false;
  }

  return activeGeneration === generation;
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
) {
  if (activeGeneration !== generation) {
    return;
  }

  refreshTimer = setTimeout(() => {
    if (activeGeneration !== generation) {
      return;
    }

    void activateCloudsync(session);
  }, delayMs);
}

async function activateCloudsync(
  session: Session,
): Promise<CloudsyncAuthChangeResult> {
  const activeGeneration = beginTransition();
  if (!(await suspendCloudsyncForGeneration(activeGeneration))) {
    if (activeGeneration === generation) {
      scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
    }
    return "ok";
  }

  const controller = new AbortController();
  exchangeController = controller;

  let response: Response;
  try {
    response = await fetch(new URL("/sync/token", env.VITE_API_URL), {
      method: "POST",
      headers: {
        Authorization: `Bearer ${session.access_token}`,
      },
      signal: controller.signal,
    });
  } catch {
    if (activeGeneration !== generation || controller.signal.aborted) {
      return "ok";
    }

    console.warn("[cloudsync] credential exchange unavailable; retrying");
    scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
    return "ok";
  } finally {
    if (exchangeController === controller) {
      exchangeController = null;
    }
  }

  if (activeGeneration !== generation) {
    return "ok";
  }

  if (!response.ok) {
    if (response.status === 404 || response.status === 501) {
      console.warn("[cloudsync] credential exchange is not configured");
      return "ok";
    }

    if (response.status === 403) {
      console.warn(
        "[cloudsync] Anarlog Pro is required; sync remains disabled",
      );
      return "ok";
    }

    if (response.status === 401) {
      console.warn("[cloudsync] credential exchange requires a fresh session");
      return "ok";
    }

    console.warn("[cloudsync] credential exchange unavailable; retrying");
    scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
    return "ok";
  }

  let credentials: unknown;
  try {
    credentials = await response.json();
  } catch {
    if (activeGeneration !== generation) {
      return "ok";
    }

    console.warn(
      "[cloudsync] credential exchange returned an invalid response",
    );
    scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
    return "ok";
  }

  if (activeGeneration !== generation) {
    return "ok";
  }

  if (!isCredentials(credentials)) {
    console.warn(
      "[cloudsync] credential exchange returned an invalid response",
    );
    scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
    return "ok";
  }

  if (credentials.workspaceId !== session.user.id) {
    console.warn(
      "[cloudsync] credential exchange returned an invalid workspace",
    );
    return "ok";
  }

  const expiresAtMs = Date.parse(credentials.expiresAt);
  if (expiresAtMs <= Date.now()) {
    console.warn("[cloudsync] credential exchange returned an expired token");
    scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
    return "ok";
  }

  try {
    const configured = await enqueuePluginOperation(async () => {
      if (activeGeneration !== generation) {
        return true;
      }

      const didConfigure = await configureCloudsyncToken(
        credentials.databaseId,
        credentials.token,
        credentials.workspaceId,
      );

      if (activeGeneration !== generation) {
        if (didConfigure) {
          await suspendCloudsync();
        }
      }

      return didConfigure;
    });

    if (activeGeneration !== generation) {
      return "ok";
    }

    if (!configured) {
      console.warn("[cloudsync] local database belongs to another account");
      return "account_mismatch";
    }
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
    scheduleExchange(session, activeGeneration, RETRY_DELAY_MS);
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
  );
  return "ok";
}

async function suspendCloudsyncSession(): Promise<void> {
  beginTransition();

  try {
    await enqueuePluginOperation(suspendCloudsync);
  } catch {
    console.warn("[cloudsync] local sync suspension failed");
  }
}

export async function prepareCloudsyncSignOut(session: Session): Promise<void> {
  const activeGeneration = beginTransition();

  try {
    await enqueuePluginOperation(() => logoutCloudsync(false));
  } catch (error) {
    scheduleExchange(session, activeGeneration, MIN_REFRESH_DELAY_MS);
    throw error;
  }
}

export async function handleCloudsyncAuthChange(
  event: AuthChangeEvent,
  session: Session | null,
): Promise<CloudsyncAuthChangeResult> {
  if (!session) {
    await suspendCloudsyncSession();
    return "ok";
  }

  if (
    event === "SIGNED_IN" ||
    event === "INITIAL_SESSION" ||
    event === "TOKEN_REFRESHED" ||
    event === "USER_UPDATED"
  ) {
    return activateCloudsync(session);
  }

  return "ok";
}
