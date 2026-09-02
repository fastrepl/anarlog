import { createServerFn, createServerOnlyFn } from "@tanstack/react-start";
import { getCookie, setCookie } from "@tanstack/react-start/server";

import { getRequestAppOrigin } from "@/functions/app-origin";
import {
  parseAuthSignInMethod,
  type AuthSignInMethod,
} from "@/lib/auth-last-sign-in-method";

const LAST_SIGN_IN_METHOD_COOKIE = "anarlog-last-sign-in-method";
const LAST_SIGN_IN_METHOD_MAX_AGE_SECONDS = 365 * 24 * 60 * 60;

export const fetchLastSignInMethod = createServerFn({ method: "GET" }).handler(
  () => parseAuthSignInMethod(getCookie(LAST_SIGN_IN_METHOD_COOKIE)),
);

export const rememberLastSignInMethod = createServerOnlyFn(
  (method: AuthSignInMethod) => {
    setCookie(LAST_SIGN_IN_METHOD_COOKIE, method, {
      httpOnly: true,
      maxAge: LAST_SIGN_IN_METHOD_MAX_AGE_SECONDS,
      path: "/",
      sameSite: "lax",
      secure: getRequestAppOrigin().startsWith("https://"),
    });
  },
);
