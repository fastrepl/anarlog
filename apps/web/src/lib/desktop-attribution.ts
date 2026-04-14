const STORAGE_KEY = "char_desktop_attribution_v2";
const MAX_AGE_MS = 24 * 60 * 60 * 1000;

type DesktopAttribution = {
  downloadIntentId: string;
  savedAt: number;
};

function readStoredAttribution() {
  if (typeof window === "undefined") {
    return null;
  }

  const rawValue = window.localStorage.getItem(STORAGE_KEY);
  if (!rawValue) {
    return null;
  }

  try {
    const parsedValue = JSON.parse(rawValue) as Partial<DesktopAttribution>;

    if (
      typeof parsedValue.downloadIntentId !== "string" ||
      typeof parsedValue.savedAt !== "number"
    ) {
      window.localStorage.removeItem(STORAGE_KEY);
      return null;
    }

    if (Date.now() - parsedValue.savedAt > MAX_AGE_MS) {
      window.localStorage.removeItem(STORAGE_KEY);
      return null;
    }

    return {
      downloadIntentId: parsedValue.downloadIntentId,
      savedAt: parsedValue.savedAt,
    };
  } catch {
    window.localStorage.removeItem(STORAGE_KEY);
    return null;
  }
}

function createDownloadIntentId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }

  return `download-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function rememberDesktopAttribution() {
  if (typeof window === "undefined") {
    return;
  }

  const attribution = {
    downloadIntentId: createDownloadIntentId(),
    savedAt: Date.now(),
  } satisfies DesktopAttribution;

  window.localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify(attribution),
  );

  return attribution;
}

export function consumeDesktopAttribution() {
  const attribution = readStoredAttribution();

  if (typeof window !== "undefined") {
    window.localStorage.removeItem(STORAGE_KEY);
  }

  return attribution;
}
