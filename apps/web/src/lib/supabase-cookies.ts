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
  const authCookieChunks = new Map<string, SupabaseRequestCookie[]>();

  for (const cookie of cookies) {
    const cookieBase = cookie.name.replace(COOKIE_CHUNK_SUFFIX, "");
    if (
      !cookieBase.startsWith("sb-") ||
      !cookieBase.endsWith(AUTH_COOKIE_SUFFIX)
    ) {
      continue;
    }
    const chunks = authCookieChunks.get(cookieBase) ?? [];
    chunks.push(cookie);
    authCookieChunks.set(cookieBase, chunks);
  }

  const invalidCookieBases = new Set<string>();
  for (const [cookieBase, chunks] of authCookieChunks) {
    const exact = chunks.find((cookie) => cookie.name === cookieBase);
    let value = exact?.value;
    if (!exact) {
      const numbered = new Map(
        chunks
          .filter((cookie) => cookie.name !== cookieBase)
          .map((cookie) => [
            Number(cookie.name.slice(cookieBase.length + 1)),
            cookie.value,
          ]),
      );
      const values: string[] = [];
      for (let index = 0; numbered.has(index); index++) {
        values.push(numbered.get(index)!);
      }
      value = values.join("");
    }
    if (!value?.startsWith(BASE64_COOKIE_PREFIX)) {
      continue;
    }
    try {
      stringFromBase64URL(value.slice(BASE64_COOKIE_PREFIX.length));
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
