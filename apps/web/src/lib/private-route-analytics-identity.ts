export const ANALYTICS_IDENTITY_COOKIE = "anlg_analytics_identity";
export const ANALYTICS_IDENTITY_MAX_AGE_SECONDS = 60 * 60 * 24 * 365;

export type AnalyticsIdentity = {
  anonymousId?: string;
  postHogId?: string;
  userId?: string;
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
    for (const key of ["anonymousId", "postHogId", "userId"] as const) {
      const value = (parsed as Record<string, unknown>)[key];
      if (typeof value === "string" && value) {
        identity[key] = value;
      }
    }
    return identity;
  } catch {
    return {};
  }
}

export function serializeAnalyticsIdentity(identity: AnalyticsIdentity) {
  return JSON.stringify(identity);
}

/**
 * Tracks which person the persisted posthog-js `distinct_id` already belongs to.
 *
 * posthog-js mints one anonymous id per browser and keeps it until `reset()`,
 * so on a shared browser the same id outlives the first sign-in. Merging it into
 * a second user would fold two people together, and keying pre-login events on it
 * would file them under the previous user. The record lives in a cookie so it
 * survives tab closes and stays visible to the server-side OAuth identify.
 */
export function createPrivateRouteIdentity(
  store: {
    read: () => AnalyticsIdentity;
    write: (identity: AnalyticsIdentity) => void;
  },
  createId: () => string = () => crypto.randomUUID(),
) {
  const sync = (postHogDistinctId: string | null): AnalyticsIdentity => {
    const identity = store.read();
    if (!postHogDistinctId || postHogDistinctId === identity.postHogId) {
      return identity;
    }

    if (postHogDistinctId === identity.userId) {
      const next = { ...identity, postHogId: postHogDistinctId };
      store.write(next);
      return next;
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
      if (identity.userId) {
        return identity.userId;
      }
      if (identity.anonymousId) {
        return identity.anonymousId;
      }

      const anonymousId = postHogDistinctId ?? createId();
      store.write({ ...identity, anonymousId });
      return anonymousId;
    },

    anonymousIdForIdentify(userId: string, postHogDistinctId: string | null) {
      const identity = sync(postHogDistinctId);
      if (identity.userId === userId) {
        return null;
      }

      const anonymousId = identity.userId
        ? createId()
        : (identity.anonymousId ?? postHogDistinctId);

      store.write({
        ...identity,
        ...(anonymousId ? { anonymousId } : {}),
        userId,
      });
      return anonymousId && anonymousId !== userId ? anonymousId : null;
    },

    /**
     * Releases the signed-in user while keeping the posthog-js id recorded as
     * claimed, so the next sign-in on this browser mints a fresh anonymous id
     * instead of merging the previous user's id into a second person.
     *
     * Returns false when nothing was ever claimed and the record can be dropped.
     */
    signOut(postHogDistinctId: string | null) {
      const identity = store.read();
      if (!identity.userId) {
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
