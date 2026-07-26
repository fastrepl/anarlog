export const DEFAULT_AUTH_RETURN_PATH = "/app/account/";

const INTERNAL_URL_ORIGIN = "https://anarlog.invalid";

export function sanitizeInternalReturnPath(value?: string) {
  if (!value?.startsWith("/") || value.startsWith("//")) {
    return DEFAULT_AUTH_RETURN_PATH;
  }

  try {
    const url = new URL(value, INTERNAL_URL_ORIGIN);
    if (url.origin !== INTERNAL_URL_ORIGIN) {
      return DEFAULT_AUTH_RETURN_PATH;
    }
    return `${url.pathname}${url.search}${url.hash}`;
  } catch {
    return DEFAULT_AUTH_RETURN_PATH;
  }
}

export function addInternalReturnPathSearch(
  returnTo: string | undefined,
  values: Record<string, string>,
) {
  const url = new URL(
    sanitizeInternalReturnPath(returnTo),
    INTERNAL_URL_ORIGIN,
  );
  for (const [name, value] of Object.entries(values)) {
    url.searchParams.set(name, value);
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

export function toAbsoluteInternalReturnUrl(
  appOrigin: string,
  returnTo: string | undefined,
) {
  return new URL(
    sanitizeInternalReturnPath(returnTo),
    new URL(appOrigin).origin,
  ).toString();
}

export function buildPostAuthDestination({
  newAccount,
  returnTo,
}: {
  newAccount: boolean;
  returnTo?: string;
}) {
  const safeReturnTo = sanitizeInternalReturnPath(returnTo);
  if (!newAccount) {
    return safeReturnTo;
  }

  const search = new URLSearchParams({
    period: "monthly",
    plan: "pro",
    trial: "true",
    source: "onboarding",
    return_to: safeReturnTo,
  });
  return `/app/checkout?${search.toString()}`;
}
