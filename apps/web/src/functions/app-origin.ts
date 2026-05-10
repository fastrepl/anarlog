import { getRequestHeaders } from "@tanstack/react-start/server";

import { env } from "@/env";

const PUBLIC_APP_HOSTS = new Set([
  "char.com",
  "www.char.com",
  "anarlog.so",
  "www.anarlog.so",
]);

const LOCAL_APP_HOSTS = new Set(["localhost", "127.0.0.1", "::1"]);

const firstHeaderValue = (value: string | null) =>
  value
    ?.split(",")
    .map((part) => part.trim())
    .find(Boolean);

export const getRequestAppOrigin = () => {
  const headers = getRequestHeaders();
  const host =
    firstHeaderValue(headers.get("x-forwarded-host")) ??
    firstHeaderValue(headers.get("host"));

  if (!host) {
    return env.VITE_APP_URL;
  }

  try {
    const parsed = new URL(`https://${host}`);
    const hostname = parsed.hostname.toLowerCase();

    if (PUBLIC_APP_HOSTS.has(hostname)) {
      return `https://${parsed.host}`;
    }

    if (import.meta.env.DEV && LOCAL_APP_HOSTS.has(hostname)) {
      return `http://${parsed.host}`;
    }
  } catch {
    return env.VITE_APP_URL;
  }

  return env.VITE_APP_URL;
};
