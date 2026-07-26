import { createServerFn } from "@tanstack/react-start";
import {
  deleteCookie,
  getCookie,
  getRequestHeaders,
  setCookie,
  setResponseHeader,
} from "@tanstack/react-start/server";
import { z } from "zod";

import { getRequestAppOrigin } from "@/functions/app-origin";
import {
  isCapabilityShareRoutePathname,
  isShareRouteToken,
  readShareRouteContinuationCookie,
} from "@/lib/share-route-privacy";

const COOKIE_PREFIX = "anarlog-share-continuation-";
const COOKIE_MAX_AGE_SECONDS = 15 * 60;

const inputSchema = z
  .object({
    pathname: z.string().min(1).max(256),
    token: z.string().regex(/^[A-Za-z0-9_-]{43}$/),
  })
  .strict();

const pathnameSchema = z.string().min(1).max(256);

export const persistShareRouteContinuation = createServerFn({ method: "POST" })
  .inputValidator(inputSchema)
  .handler(async ({ data }) => {
    setPrivateContinuationResponseHeaders();
    if (!isSameOriginContinuationRequest()) {
      return false;
    }

    const pathname = canonicalCapabilityPathname(data.pathname);
    if (!pathname || !isShareRouteToken(data.token)) {
      return false;
    }

    const options = continuationCookieOptions();
    setCookie(await continuationCookieName(pathname), data.token, options);
    return true;
  });

export const restoreShareRouteContinuation = createServerFn({ method: "POST" })
  .inputValidator(pathnameSchema)
  .handler(async ({ data }) => {
    setPrivateContinuationResponseHeaders();
    if (!isSameOriginContinuationRequest()) {
      return null;
    }

    const requestedPathname = canonicalCapabilityPathname(data);
    if (!requestedPathname) {
      return null;
    }

    const cookieName = await continuationCookieName(requestedPathname);
    return readShareRouteContinuationCookie(getCookie(cookieName), () => {
      clearContinuationCookie(cookieName);
    });
  });

export const clearShareRouteContinuation = createServerFn({
  method: "POST",
})
  .inputValidator(pathnameSchema)
  .handler(async ({ data }) => {
    setPrivateContinuationResponseHeaders();
    if (!isSameOriginContinuationRequest()) {
      return false;
    }

    const pathname = canonicalCapabilityPathname(data);
    if (!pathname) {
      return false;
    }

    clearContinuationCookie(await continuationCookieName(pathname));
    return true;
  });

function canonicalCapabilityPathname(pathname: string) {
  const canonical =
    pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
  return isCapabilityShareRoutePathname(canonical) ? canonical : null;
}

function continuationCookieOptions() {
  return {
    httpOnly: true,
    maxAge: COOKIE_MAX_AGE_SECONDS,
    path: "/",
    sameSite: "lax" as const,
    secure: getRequestAppOrigin().startsWith("https://"),
  };
}

function clearContinuationCookie(cookieName: string) {
  const options = { path: "/" };
  deleteCookie(cookieName, options);
}

async function continuationCookieName(pathname: string) {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(pathname),
  );
  const suffix = Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
  return `${COOKIE_PREFIX}${suffix}`;
}

function isSameOriginContinuationRequest() {
  const headers = getRequestHeaders();
  const origin = headers.get("origin");
  if (origin) {
    try {
      return new URL(origin).origin === getRequestAppOrigin();
    } catch {
      return false;
    }
  }

  return headers.get("sec-fetch-site") === "same-origin";
}

function setPrivateContinuationResponseHeaders() {
  setResponseHeader("Cache-Control", "private, no-store");
  setResponseHeader("Referrer-Policy", "no-referrer");
  setResponseHeader("X-Robots-Tag", "noindex, nofollow, noarchive, nosnippet");
}
