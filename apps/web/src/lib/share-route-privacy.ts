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

export function prepareShareRoutePrivacy() {
  if (typeof window === "undefined") {
    return;
  }

  const { hash, pathname, search } = window.location;
  if (!isShareRoutePathname(pathname)) {
    return;
  }

  if (isCapabilityShareRoutePathname(pathname)) {
    const fragmentToken = parseShareFragmentToken(hash);
    if (fragmentToken) {
      inMemoryToken = { pathname, token: fragmentToken };
      try {
        window.sessionStorage.setItem(
          SHARE_TOKEN_STORAGE_KEY,
          JSON.stringify(inMemoryToken),
        );
      } catch {
        // The in-memory copy still supports the current page when storage is unavailable.
      }
    } else {
      inMemoryToken = readStoredToken(pathname);
    }
  }

  if (hash) {
    window.history.replaceState(null, "", `${pathname}${search}`);
  }
}

export function getShareRouteToken(pathname: string): string | null {
  if (inMemoryToken?.pathname === pathname) {
    return inMemoryToken.token;
  }

  inMemoryToken = readStoredToken(pathname);
  return inMemoryToken?.token ?? null;
}

export function clearShareRouteToken(pathname: string) {
  if (inMemoryToken?.pathname === pathname) {
    inMemoryToken = null;
  }

  if (typeof window === "undefined") {
    return;
  }

  try {
    const stored = parseStoredToken(
      window.sessionStorage.getItem(SHARE_TOKEN_STORAGE_KEY),
    );
    if (stored?.pathname === pathname) {
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
    return stored?.pathname === pathname ? stored : null;
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
      return { pathname: parsed.pathname, token: parsed.token };
    }
  } catch {
    return null;
  }

  return null;
}
