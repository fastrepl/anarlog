const DEFAULT_RECENT_AUTH_CALLBACK_WINDOW_MS = 5_000;

export function createAuthCallbackHandler({
  setSessionFromTokens,
  now = Date.now,
  recentWindowMs = DEFAULT_RECENT_AUTH_CALLBACK_WINDOW_MS,
}: {
  setSessionFromTokens: (
    accessToken: string,
    refreshToken: string,
  ) => Promise<void>;
  now?: () => number;
  recentWindowMs?: number;
}) {
  const inFlight = new Set<string>();
  const recent = new Map<string, number>();

  return (accessToken: string, refreshToken: string) => {
    const timestamp = now();

    for (const [key, completedAt] of recent) {
      if (timestamp - completedAt >= recentWindowMs) {
        recent.delete(key);
      }
    }

    const key = JSON.stringify([accessToken, refreshToken]);
    if (inFlight.has(key) || recent.has(key)) {
      return false;
    }

    inFlight.add(key);
    void setSessionFromTokens(accessToken, refreshToken).then(
      () => {
        inFlight.delete(key);
        recent.set(key, now());
      },
      () => {
        inFlight.delete(key);
      },
    );

    return true;
  };
}
