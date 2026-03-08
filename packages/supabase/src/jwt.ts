import { createRemoteJWKSet, jwtVerify } from "jose";

// Mirrors crates/supabase-auth/src/claims.rs SubscriptionStatus
export type SubscriptionStatus =
  | "incomplete"
  | "incomplete_expired"
  | "trialing"
  | "active"
  | "past_due"
  | "canceled"
  | "unpaid"
  | "paused";

export type SupabaseJwtPayload = {
  sub?: string;
  email?: string;
  entitlements?: string[];
  subscription_status?: SubscriptionStatus | null;
  trial_end?: number | null;
};

export type JwksVerifier = {
  verify: (token: string) => Promise<SupabaseJwtPayload>;
};

export function createJwksVerifier(supabaseUrl: string): JwksVerifier {
  const jwksUrl = new URL("/auth/v1/.well-known/jwks.json", supabaseUrl);
  const jwks = createRemoteJWKSet(jwksUrl, {
    cacheMaxAge: 600_000,
  });

  return {
    verify: async (token: string) => {
      const { payload } = await jwtVerify<SupabaseJwtPayload>(token, jwks, {
        audience: "authenticated",
        issuer: `${supabaseUrl}/auth/v1`,
      });
      return payload;
    },
  };
}
