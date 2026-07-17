const SHARE_TOKEN_STORAGE_KEY = "anarlog.share-route-token";
const SHARE_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;

let inMemoryToken: { pathname: string; token: string } | null = null;

export function isShareRoutePathname(pathname: string) {
  return pathname === "/share" || pathname.startsWith("/share/");
}

export function isCapabilityShareRoutePathname(pathname: string) {
  return (
    /^\/share\/invite\/[^/]+\/?$/.test(pathname) ||
    /^\/share\/link\/[^/]+\/?$/.test(pathname)
  );
}

export function parseShareFragmentToken(hash: string): string | null {
  if (!hash.startsWith("#")) {
    return null;
  }

  const entries = [...new URLSearchParams(hash.slice(1)).entries()];
  if (entries.length !== 1 || entries[0]?.[0] !== "token") {
    return null;
  }

  const token = entries[0][1];
  return isShareRouteToken(token) ? token : null;
}

export function isShareRouteToken(value: string) {
  return SHARE_TOKEN_PATTERN.test(value);
}

export function readShareRouteContinuationCookie(
  value: string | undefined,
  clearInvalid: () => void,
) {
  if (!value) {
    return null;
  }
  if (!isShareRouteToken(value)) {
    clearInvalid();
    return null;
  }
  return value;
}

export async function loadShareRouteContinuation({
  clearPersisted,
  localToken,
  persist,
  restore,
  retain,
  signal,
}: {
  clearPersisted: () => void;
  localToken: string | null;
  persist: (token: string) => Promise<boolean>;
  restore: () => Promise<string | null>;
  retain: (token: string) => boolean;
  signal?: AbortSignal;
}) {
  signal?.throwIfAborted();
  let token = localToken;
  if (!token) {
    token = await restore();
    signal?.throwIfAborted();
    if (!token) {
      return null;
    }
    if (!retain(token)) {
      throw new Error("share continuation unavailable");
    }
  }

  const persisted = await persist(token);
  signal?.throwIfAborted();
  if (!persisted) {
    throw new Error("share continuation unavailable");
  }

  clearPersisted();
  return token;
}

export function prepareShareRoutePrivacy() {
  if (typeof window === "undefined") {
    return;
  }

  const { hash, pathname, search } = window.location;
  if (!isShareRoutePathname(pathname)) {
    return;
  }

  if (isCapabilityShareRoutePathname(pathname)) {
    const tokenPathname = canonicalShareRoutePathname(pathname);
    const fragmentToken = parseShareFragmentToken(hash);
    if (fragmentToken) {
      inMemoryToken = { pathname: tokenPathname, token: fragmentToken };
      try {
        window.sessionStorage.setItem(
          SHARE_TOKEN_STORAGE_KEY,
          JSON.stringify(inMemoryToken),
        );
      } catch {
        // The in-memory copy still supports the current page when storage is unavailable.
      }
    } else {
      inMemoryToken = readStoredToken(tokenPathname);
    }
  }

  if (hash) {
    window.history.replaceState(null, "", `${pathname}${search}`);
  }
}

export function getShareRouteToken(pathname: string): string | null {
  const tokenPathname = canonicalShareRoutePathname(pathname);
  if (inMemoryToken?.pathname === tokenPathname) {
    return inMemoryToken.token;
  }

  inMemoryToken = readStoredToken(tokenPathname);
  return inMemoryToken?.token ?? null;
}

export function retainShareRouteToken(pathname: string, token: string) {
  if (typeof window === "undefined" || !isShareRouteToken(token)) {
    return false;
  }

  const tokenPathname = canonicalShareRoutePathname(pathname);
  if (!isCapabilityShareRoutePathname(tokenPathname)) {
    return false;
  }

  inMemoryToken = { pathname: tokenPathname, token };
  try {
    window.sessionStorage.setItem(
      SHARE_TOKEN_STORAGE_KEY,
      JSON.stringify(inMemoryToken),
    );
  } catch {
    // The in-memory copy still supports the current page when storage is unavailable.
  }
  return true;
}

export function clearShareRouteToken(pathname: string) {
  const tokenPathname = canonicalShareRoutePathname(pathname);
  if (inMemoryToken?.pathname === tokenPathname) {
    inMemoryToken = null;
  }

  clearPersistedShareRouteToken(pathname);
}

export function clearPersistedShareRouteToken(pathname: string) {
  const tokenPathname = canonicalShareRoutePathname(pathname);

  if (typeof window === "undefined") {
    return;
  }

  try {
    const stored = parseStoredToken(
      window.sessionStorage.getItem(SHARE_TOKEN_STORAGE_KEY),
    );
    if (stored?.pathname === tokenPathname) {
      window.sessionStorage.removeItem(SHARE_TOKEN_STORAGE_KEY);
    }
  } catch {
    // Storage cleanup is best-effort.
  }
}

function readStoredToken(pathname: string) {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    const stored = parseStoredToken(
      window.sessionStorage.getItem(SHARE_TOKEN_STORAGE_KEY),
    );
    return stored?.pathname === canonicalShareRoutePathname(pathname)
      ? stored
      : null;
  } catch {
    return null;
  }
}

function parseStoredToken(value: string | null) {
  if (!value) {
    return null;
  }

  try {
    const parsed = JSON.parse(value) as unknown;
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "pathname" in parsed &&
      "token" in parsed &&
      typeof parsed.pathname === "string" &&
      typeof parsed.token === "string" &&
      isCapabilityShareRoutePathname(parsed.pathname) &&
      isShareRouteToken(parsed.token)
    ) {
      return {
        pathname: canonicalShareRoutePathname(parsed.pathname),
        token: parsed.token,
      };
    }
  } catch {
    return null;
  }

  return null;
}

function canonicalShareRoutePathname(pathname: string) {
  return pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
}
