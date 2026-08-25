import { z } from "zod";

export const oauthAuthorizationIdSchema = z
  .string()
  .min(1)
  .max(256)
  .regex(/^[A-Za-z0-9_-]+$/);

const scopeDescriptions: Record<string, string> = {
  openid: "Confirm your Anarlog account identity",
  email: "Share your Anarlog account email with the connector",
  offline_access: "Stay connected without asking you to sign in every time",
};

export function describeOAuthScopes(scope: string) {
  return scope
    .split(/\s+/)
    .filter(Boolean)
    .map(
      (value) => scopeDescriptions[value] ?? `Grant the ${value} permission`,
    );
}
