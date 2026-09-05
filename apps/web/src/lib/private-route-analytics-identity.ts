// Legacy browser IDs may already be linked to accounts, even if locally anonymous.
export const ANALYTICS_IDENTITY_COOKIE = "anlg_analytics_identity_v2";
export const ANALYTICS_IDENTITY_MAX_AGE_SECONDS = 60 * 60 * 24 * 365;

export function getPostHogPersistenceName(apiKey: string) {
  return `${apiKey}_anonymous_v2`;
}

export type AnalyticsIdentity = {
  anonymousId?: string;
  legacyIdentified?: boolean;
  postHogId?: string;
};

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

export function parseAnalyticsIdentity(raw: string | null | undefined) {
  if (!raw) {
    return {};
  }

  try {
    const parsed = JSON.parse(raw) as unknown;
    if (typeof parsed !== "object" || parsed === null) {
      return {};
    }

    const identity: AnalyticsIdentity = {};
    for (const key of ["anonymousId", "postHogId"] as const) {
      const value = (parsed as Record<string, unknown>)[key];
      if (typeof value === "string" && value) {
        identity[key] = value;
      }
    }
    if (
      typeof (parsed as Record<string, unknown>).userId === "string" &&
      (parsed as Record<string, unknown>).userId
    ) {
      identity.legacyIdentified = true;
    }
    return identity;
  } catch {
    return {};
  }
}

export function serializeAnalyticsIdentity(identity: AnalyticsIdentity) {
  const { anonymousId, postHogId } = identity;
  return JSON.stringify({
    ...(anonymousId ? { anonymousId } : {}),
    ...(postHogId ? { postHogId } : {}),
  });
}

/** Keeps private-route events on an anonymous browser identity. */
export function createPrivateRouteIdentity(
  store: {
    read: () => AnalyticsIdentity;
    write: (identity: AnalyticsIdentity) => void;
  },
  createId: () => string = () => crypto.randomUUID(),
) {
  const sync = (postHogDistinctId: string | null): AnalyticsIdentity => {
    const identity = store.read();
    if (identity.legacyIdentified) {
      const next = {
        anonymousId: createId(),
        ...(postHogDistinctId ? { postHogId: postHogDistinctId } : {}),
      };
      store.write(next);
      return next;
    }
    if (!postHogDistinctId || postHogDistinctId === identity.postHogId) {
      return identity;
    }

    const next = {
      anonymousId: postHogDistinctId,
      postHogId: postHogDistinctId,
    };
    store.write(next);
    return next;
  };

  return {
    distinctIdForEvent(postHogDistinctId: string | null) {
      const identity = sync(postHogDistinctId);
      if (identity.anonymousId) {
        return identity.anonymousId;
      }

      const anonymousId = postHogDistinctId ?? createId();
      store.write({ ...identity, anonymousId });
      return anonymousId;
    },

    /** Rotates the anonymous identity when an authenticated session ends. */
    signOut(postHogDistinctId: string | null) {
      const identity = store.read();
      if (!identity.postHogId && !identity.legacyIdentified) {
        return false;
      }

      const claimedPostHogId = postHogDistinctId ?? identity.postHogId;
      if (!claimedPostHogId) {
        return false;
      }

      store.write({
        anonymousId: createId(),
        postHogId: claimedPostHogId,
      });
      return true;
    },

    reset() {
      store.write({});
    },
  };
}
