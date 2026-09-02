import { emitTo, listen } from "@tauri-apps/api/event";

import { id } from "~/shared/utils";

export const AUTH_SIGN_OUT_REQUEST_EVENT = "anlg:auth-sign-out-request";
export const AUTH_SIGN_OUT_RESULT_EVENT = "anlg:auth-sign-out-result";
export const AUTH_SIGN_OUT_COMMITTED_EVENT = "anlg:auth-sign-out-committed";
const AUTH_SIGN_OUT_TIMEOUT_MS = 10_000;

export type AuthSignOutRequestPayload = {
  requestId: string;
  sourceLabel: string;
};

export type AuthSignOutResultPayload = {
  requestId: string;
  completed: boolean;
  error: string | null;
};

export type AuthSignOutCommittedPayload = {
  sourceLabel: string;
};

export function isAuthSignOutCommittedPayload(
  payload: unknown,
): payload is AuthSignOutCommittedPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<AuthSignOutCommittedPayload>;
  return (
    typeof candidate.sourceLabel === "string" &&
    candidate.sourceLabel.length > 0
  );
}

export function isAuthSignOutRequestPayload(
  payload: unknown,
): payload is AuthSignOutRequestPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<AuthSignOutRequestPayload>;
  return (
    typeof candidate.requestId === "string" &&
    candidate.requestId.length > 0 &&
    typeof candidate.sourceLabel === "string" &&
    candidate.sourceLabel.length > 0
  );
}

function isAuthSignOutResultPayload(
  payload: unknown,
): payload is AuthSignOutResultPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<AuthSignOutResultPayload>;
  return (
    typeof candidate.requestId === "string" &&
    typeof candidate.completed === "boolean" &&
    (candidate.error === null || typeof candidate.error === "string")
  );
}

export function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export async function requestMainSignOut(
  sourceLabel: string,
): Promise<boolean> {
  const requestId = id();
  let timeout: ReturnType<typeof setTimeout> | null = null;
  let resolveResult!: (completed: boolean) => void;
  let rejectResult!: (error: Error) => void;
  const result = new Promise<boolean>((resolve, reject) => {
    resolveResult = resolve;
    rejectResult = reject;
  });
  const unlisten = await listen<AuthSignOutResultPayload>(
    AUTH_SIGN_OUT_RESULT_EVENT,
    (event) => {
      if (
        !isAuthSignOutResultPayload(event.payload) ||
        event.payload.requestId !== requestId
      ) {
        return;
      }

      if (event.payload.error) {
        rejectResult(new Error(event.payload.error));
      } else {
        resolveResult(event.payload.completed);
      }
    },
  );

  timeout = setTimeout(() => {
    rejectResult(new Error("Main window did not acknowledge sign-out"));
  }, AUTH_SIGN_OUT_TIMEOUT_MS);

  try {
    const [, completed] = await Promise.all([
      emitTo("main", AUTH_SIGN_OUT_REQUEST_EVENT, {
        requestId,
        sourceLabel,
      } satisfies AuthSignOutRequestPayload),
      result,
    ]);
    return completed;
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
    unlisten();
  }
}
