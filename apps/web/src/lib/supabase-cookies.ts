import { stringFromBase64URL } from "@supabase/ssr";

type SameSite = "lax" | "strict" | "none";

const BASE64_COOKIE_PREFIX = "base64-";
const COOKIE_CHUNK_SUFFIX = /\.\d+$/;
const AUTH_COOKIE_SUFFIX = "-auth-token";

type SupabaseRequestCookie = { name: string; value: string };

export type SupabaseCookie = {
  name: string;
  value: string;
  options?: {
    domain?: string;
    expires?: Date;
    httpOnly?: boolean;
    maxAge?: number;
    path?: string;
    sameSite?: SameSite | boolean;
    secure?: boolean;
  };
};

/**
 * A damaged SSR auth cookie must behave like a missing session. Supabase's
 * decoder intentionally throws for invalid UTF-8, which otherwise turns an
 * anonymous request into a server error before the route can handle it.
 */
export function filterInvalidSupabaseCookies(cookies: SupabaseRequestCookie[]) {
  const invalidCookieBases = new Set<string>();

  for (const cookie of cookies) {
    const cookieBase = cookie.name.replace(COOKIE_CHUNK_SUFFIX, "");
    if (
      !cookieBase.startsWith("sb-") ||
      !cookieBase.endsWith(AUTH_COOKIE_SUFFIX) ||
      !cookie.value.startsWith(BASE64_COOKIE_PREFIX)
    ) {
      continue;
    }

    try {
      stringFromBase64URL(cookie.value.slice(BASE64_COOKIE_PREFIX.length));
    } catch {
      invalidCookieBases.add(cookieBase);
    }
  }

  if (invalidCookieBases.size === 0) {
    return cookies;
  }

  return cookies.filter(
    (cookie) =>
      !invalidCookieBases.has(cookie.name.replace(COOKIE_CHUNK_SUFFIX, "")),
  );
}

export function toSetCookieOptions(cookie: SupabaseCookie) {
  const sameSite = cookie.options?.sameSite;
  return {
    domain: cookie.options?.domain,
    expires: cookie.options?.expires,
    httpOnly: cookie.options?.httpOnly,
    maxAge: cookie.options?.maxAge,
    path: cookie.options?.path ?? "/",
    sameSite:
      sameSite === true
        ? ("strict" as const)
        : sameSite === false
          ? undefined
          : sameSite,
    secure: cookie.options?.secure,
  };
}
