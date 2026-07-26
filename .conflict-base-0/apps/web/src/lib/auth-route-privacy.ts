import { isShareRoutePathname } from "./share-route-privacy.ts";

const AUTH_HANDOFF_STORAGE_KEY = "anarlog.desktop-auth-handoff";
const AUTH_HANDOFF_MAX_AGE_MS = 5 * 60 * 1000;
const AUTH_CALLBACK_SEARCH_KEYS = [
  "access_token",
  "refresh_token",
  "code",
  "token_hash",
  "error",
] as const;
const AUTH_HANDOFF_SEARCH_KEYS = ["access_token", "refresh_token"] as const;
const AUTH_PRIVATE_PATHS = new Set([
  "/auth",
  "/callback/auth",
  "/confirm-auth",
  "/reset-password",
  "/update-password",
]);

type SearchValue =
  | string
  | URLSearchParams
  | Record<string, unknown>
  | undefined;

export function isTelemetryPrivateLocation(
  pathname: string,
  search?: SearchValue,
) {
  const canonicalPathname = canonicalPath(pathname);
  if (
    isShareRoutePathname(canonicalPathname) ||
    AUTH_PRIVATE_PATHS.has(canonicalPathname)
  ) {
    return true;
  }

  return (
    canonicalPathname === "/" &&
    AUTH_CALLBACK_SEARCH_KEYS.some((key) => hasSearchKey(search, key))
  );
}

export function prepareAuthRoutePrivacy(now = Date.now()) {
  if (typeof window === "undefined") {
    return;
  }

  const { hash, pathname, search } = window.location;
  if (!isTelemetryPrivateLocation(pathname, search)) {
    return;
  }

  const params = new URLSearchParams(search);
  const accessToken = params.get("access_token");
  const refreshToken = params.get("refresh_token");
  if (
    canonicalPath(pathname) !== "/callback/auth" ||
    !accessToken ||
    !refreshToken
  ) {
    return;
  }

  try {
    window.sessionStorage.setItem(
      AUTH_HANDOFF_STORAGE_KEY,
      JSON.stringify({ accessToken, refreshToken, storedAt: now }),
    );
  } catch {
    return;
  }

  params.set("handoff", "stored");
  for (const key of AUTH_HANDOFF_SEARCH_KEYS) {
    params.delete(key);
  }

  const sanitizedSearch = params.toString();
  window.history.replaceState(
    window.history.state,
    "",
    `${pathname}${sanitizedSearch ? `?${sanitizedSearch}` : ""}${hash}`,
  );
}

export function consumeDesktopAuthHandoff(now = Date.now()) {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    const stored = window.sessionStorage.getItem(AUTH_HANDOFF_STORAGE_KEY);
    if (!stored) {
      return null;
    }

    window.sessionStorage.removeItem(AUTH_HANDOFF_STORAGE_KEY);
    const parsed = JSON.parse(stored) as unknown;
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "accessToken" in parsed &&
      "refreshToken" in parsed &&
      "storedAt" in parsed &&
      typeof parsed.accessToken === "string" &&
      typeof parsed.refreshToken === "string" &&
      typeof parsed.storedAt === "number"
    ) {
      const age = now - parsed.storedAt;
      if (age < 0 || age > AUTH_HANDOFF_MAX_AGE_MS) {
        return null;
      }

      return {
        accessToken: parsed.accessToken,
        refreshToken: parsed.refreshToken,
      };
    }
  } catch {
    return null;
  }

  return null;
}

function canonicalPath(pathname: string) {
  return pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
}

function hasSearchKey(search: SearchValue, key: string) {
  if (!search) {
    return false;
  }
  if (typeof search === "string") {
    return new URLSearchParams(search).has(key);
  }
  if (search instanceof URLSearchParams) {
    return search.has(key);
  }
  return key in search;
}
