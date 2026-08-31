export const authSignInMethods = [
  "apple",
  "google",
  "azure",
  "github",
  "email",
  "sso",
] as const;

export type AuthSignInMethod = (typeof authSignInMethods)[number];

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

export function resolveSignInMethod({
  attemptedMethod,
  provider,
  usesSso,
}: {
  attemptedMethod?: AuthSignInMethod;
  provider: unknown;
  usesSso: boolean;
}): AuthSignInMethod | null {
  return attemptedMethod ?? (usesSso ? "sso" : parseAuthSignInMethod(provider));
}

export function shouldRememberOtpSignIn(type: string) {
  return type !== "recovery" && type !== "email_change";
}
