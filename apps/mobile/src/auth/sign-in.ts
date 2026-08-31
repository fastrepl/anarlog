export type SignInMethod =
  | "apple"
  | "google"
  | "azure"
  | "github"
  | "email"
  | "sso";

export const lastSignInMethodStorageKey = "anarlog:auth:last-sign-in-method";

export function parseLastSignInMethod(
  value: string | null,
): SignInMethod | null {
  switch (value) {
    case "apple":
    case "google":
    case "azure":
    case "github":
    case "email":
    case "sso":
      return value;
    default:
      return null;
  }
}

export function parseAuthCallbackSignInMethod(
  callbackUrl: string,
): SignInMethod | null {
  try {
    return parseLastSignInMethod(
      new URL(callbackUrl).searchParams.get("method"),
    );
  } catch {
    return null;
  }
}

export function buildSignInUrl(appUrl: string, method: SignInMethod): string {
  const url = new URL(`${appUrl.replace(/\/+$/, "")}/auth`);
  url.searchParams.set("flow", "desktop");
  url.searchParams.set("scheme", "anarlog");

  if (method === "email" || method === "sso") {
    url.searchParams.set("view", method);
  } else {
    url.searchParams.set("provider", method);
  }

  return url.toString();
}
