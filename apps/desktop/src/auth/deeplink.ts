const PENDING_AUTH_KEY = "anarlog.pending-desktop-auth";
const installingStates = new Set<string>();
const PENDING_AUTH_MAX_AGE_MS = 15 * 60 * 1000;

export function beginDesktopAuth(now = Date.now()) {
  const state = crypto.randomUUID();
  localStorage.setItem(
    PENDING_AUTH_KEY,
    JSON.stringify({ state, createdAt: now }),
  );
  return state;
}

function pendingDesktopAuthState(
  state: string | null | undefined,
  now = Date.now(),
) {
  if (!state) return false;
  try {
    const stored = localStorage.getItem(PENDING_AUTH_KEY);
    if (!stored) return false;
    const pending = JSON.parse(stored);
    if (
      pending.state !== state ||
      typeof pending.createdAt !== "number" ||
      now < pending.createdAt ||
      now - pending.createdAt > PENDING_AUTH_MAX_AGE_MS
    )
      return false;
    return stored;
  } catch {
    return false;
  }
}

export function consumeDesktopAuthState(
  state: string | null | undefined,
  now = Date.now(),
) {
  if (!pendingDesktopAuthState(state, now)) return false;
  localStorage.removeItem(PENDING_AUTH_KEY);
  return true;
}

export async function installDesktopAuthSession(
  accessToken: string,
  refreshToken: string,
  state: string | null | undefined,
  install: (accessToken: string, refreshToken: string) => Promise<void>,
) {
  if (!accessToken || !refreshToken || !state || installingStates.has(state))
    return false;
  const pending = pendingDesktopAuthState(state);
  if (!pending) return false;
  installingStates.add(state);
  try {
    await install(accessToken, refreshToken);
    if (localStorage.getItem(PENDING_AUTH_KEY) === pending)
      localStorage.removeItem(PENDING_AUTH_KEY);
    return true;
  } finally {
    installingStates.delete(state);
  }
}

export function createAuthCallbackHandler({
  setSessionFromTokens,
}: {
  setSessionFromTokens: (
    accessToken: string,
    refreshToken: string,
  ) => Promise<void>;
}) {
  return (accessToken: string, refreshToken: string, state?: string) =>
    installDesktopAuthSession(
      accessToken,
      refreshToken,
      state,
      setSessionFromTokens,
    );
}
