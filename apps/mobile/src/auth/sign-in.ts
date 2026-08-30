export type SignInMethod =
  | "apple"
  | "google"
  | "azure"
  | "github"
  | "email"
  | "sso";

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
