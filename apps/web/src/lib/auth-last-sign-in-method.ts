export type AuthSignInMethod =
  | "apple"
  | "google"
  | "azure"
  | "github"
  | "email"
  | "sso";

export function parseAuthSignInMethod(value: unknown): AuthSignInMethod | null {
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

export function resolveSessionSignInMethod({
  provider,
  usesSso,
}: {
  provider: unknown;
  usesSso: boolean;
}): AuthSignInMethod | null {
  return usesSso ? "sso" : parseAuthSignInMethod(provider);
}

export function shouldRememberOtpSignIn(type: string) {
  return type !== "recovery" && type !== "email_change";
}
