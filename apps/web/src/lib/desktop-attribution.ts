const STORAGE_KEY = "char_desktop_attribution_v1";
const MAX_AGE_MS = 30 * 24 * 60 * 60 * 1000;

function readStoredDistinctId() {
  if (typeof window === "undefined") {
    return null;
  }

  const rawValue = window.localStorage.getItem(STORAGE_KEY);
  if (!rawValue) {
    return null;
  }

  try {
    const parsedValue = JSON.parse(rawValue) as Partial<{
      distinctId: string;
      savedAt: number;
    }>;

    if (
      typeof parsedValue.distinctId !== "string" ||
      typeof parsedValue.savedAt !== "number"
    ) {
      window.localStorage.removeItem(STORAGE_KEY);
      return null;
    }

    if (Date.now() - parsedValue.savedAt > MAX_AGE_MS) {
      window.localStorage.removeItem(STORAGE_KEY);
      return null;
    }

    return parsedValue.distinctId;
  } catch {
    window.localStorage.removeItem(STORAGE_KEY);
    return null;
  }
}

export function rememberDesktopAttributionDistinctId(
  distinctId: string | null | undefined,
) {
  if (typeof window === "undefined" || !distinctId) {
    return;
  }

  window.localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({
      distinctId,
      savedAt: Date.now(),
    }),
  );
}

export function getDesktopAttributionDistinctId(
  currentDistinctId: string | null | undefined,
) {
  return readStoredDistinctId() ?? currentDistinctId ?? null;
}

export function getDesktopAttributionAliasCandidates(
  currentDistinctId: string | null | undefined,
) {
  return [
    ...new Set(
      [readStoredDistinctId(), currentDistinctId].filter(
        (distinctId): distinctId is string => Boolean(distinctId),
      ),
    ),
  ];
}
