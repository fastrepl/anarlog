const ANONYMOUS_ID_KEY = "anarlog.private-analytics-anonymous-id";
const POSTHOG_ID_KEY = "anarlog.private-analytics-posthog-id";
const USER_ID_KEY = "anarlog.private-analytics-user-id";

export function parsePostHogDistinctId(raw: string) {
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "distinct_id" in parsed &&
      typeof parsed.distinct_id === "string" &&
      parsed.distinct_id
    ) {
      return parsed.distinct_id;
    }
  } catch {}

  return null;
}

export function createPrivateRouteIdentity(
  createId: () => string = () => crypto.randomUUID(),
) {
  let fallbackAnonymousId: string | null = null;
  let fallbackPostHogId: string | null = null;
  let fallbackUserId: string | null = null;

  const read = (key: string) => {
    try {
      return window.sessionStorage.getItem(key);
    } catch {
      return null;
    }
  };

  const write = (key: string, value: string) => {
    try {
      window.sessionStorage.setItem(key, value);
    } catch {}
  };

  const clear = (key: string) => {
    try {
      window.sessionStorage.removeItem(key);
    } catch {}
  };

  const getAnonymousId = (postHogDistinctId: string | null) => {
    const existing = read(ANONYMOUS_ID_KEY) ?? fallbackAnonymousId;
    if (existing) {
      return existing;
    }

    const resolved = postHogDistinctId ?? createId();
    fallbackAnonymousId = resolved;
    write(ANONYMOUS_ID_KEY, resolved);
    return resolved;
  };

  const setAnonymousId = (anonymousId: string) => {
    fallbackAnonymousId = anonymousId;
    write(ANONYMOUS_ID_KEY, anonymousId);
  };

  const getPostHogId = () => read(POSTHOG_ID_KEY) ?? fallbackPostHogId;

  const setPostHogId = (postHogId: string) => {
    fallbackPostHogId = postHogId;
    write(POSTHOG_ID_KEY, postHogId);
  };

  const getUserId = () => read(USER_ID_KEY) ?? fallbackUserId;

  const setUserId = (userId: string) => {
    fallbackUserId = userId;
    write(USER_ID_KEY, userId);
  };

  const clearUserId = () => {
    fallbackUserId = null;
    clear(USER_ID_KEY);
  };

  const syncPostHogId = (postHogDistinctId: string | null) => {
    if (!postHogDistinctId || postHogDistinctId === getPostHogId()) {
      return;
    }

    setPostHogId(postHogDistinctId);
    const userId = getUserId();
    if (postHogDistinctId === userId) {
      return;
    }

    setAnonymousId(postHogDistinctId);
    if (userId) {
      clearUserId();
    }
  };

  return {
    distinctIdForEvent(postHogDistinctId: string | null) {
      syncPostHogId(postHogDistinctId);
      return getUserId() ?? getAnonymousId(postHogDistinctId);
    },

    anonymousIdForIdentify(userId: string, postHogDistinctId: string | null) {
      syncPostHogId(postHogDistinctId);
      const previousUserId = getUserId();
      if (previousUserId === userId) {
        return null;
      }

      const previousAnonymousId = getAnonymousId(postHogDistinctId);
      const anonymousId = previousUserId ? createId() : previousAnonymousId;

      setAnonymousId(anonymousId);
      setUserId(userId);
      return anonymousId === userId ? null : anonymousId;
    },
  };
}
