const PENDING_AUTH_KEY = "anarlog.pending-desktop-auth";
const PENDING_AUTH_MAX_AGE_MS = 15 * 60 * 1000;

export function beginDesktopAuth(now = Date.now()) {
  const state = crypto.randomUUID();
  localStorage.setItem(
    PENDING_AUTH_KEY,
    JSON.stringify({ state, createdAt: now }),
  );
  return state;
}

export function consumeDesktopAuthState(
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
    // Consume before installing the session so queued callbacks cannot replay it.
    localStorage.removeItem(PENDING_AUTH_KEY);
    return true;
  } catch {
    return false;
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
  return (accessToken: string, refreshToken: string, state?: string) => {
    if (!accessToken || !refreshToken || !consumeDesktopAuthState(state))
      return false;
    void setSessionFromTokens(accessToken, refreshToken).catch(() => {});
    return true;
  };
}
