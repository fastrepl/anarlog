type SameSite = "lax" | "strict" | "none";

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
