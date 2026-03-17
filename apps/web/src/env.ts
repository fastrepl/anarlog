import { createEnv } from "@t3-oss/env-core";
import { z } from "zod";

const isCI = process.env.CI === "true";
const isDev = process.env.NODE_ENV !== "production";

const requiredInProd = <T extends z.ZodTypeAny>(schema: T) =>
  isDev ? schema.optional() : schema;

export function requireEnv<T>(value: T | undefined, name: string): T {
  if (value === undefined) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

export const env = createEnv({
  server: {
    NANGO_SECRET_KEY: requiredInProd(z.string().min(1)),
    DATABASE_URL: requiredInProd(z.string().min(1)),

    SUPABASE_URL: requiredInProd(z.string().min(1)),
    SUPABASE_ANON_KEY: requiredInProd(z.string().min(1)),
    SUPABASE_SERVICE_ROLE_KEY: z.string().min(1).optional(),

    STRIPE_SECRET_KEY: requiredInProd(z.string().min(1)),
    STRIPE_MONTHLY_PRICE_ID: requiredInProd(z.string().min(1)),
    STRIPE_YEARLY_PRICE_ID: requiredInProd(z.string().min(1)),

    LOOPS_KEY: requiredInProd(z.string().min(1)),

    GITHUB_TOKEN: z.string().optional(),
  },

  clientPrefix: "VITE_",
  client: {
    VITE_APP_URL: isDev
      ? z.string().default("http://localhost:3000")
      : z.string().min(1),
    VITE_API_URL: isDev
      ? z.string().default("http://localhost:3001")
      : z.string().default("https://api.hyprnote.com"),
    VITE_SUPABASE_URL: requiredInProd(z.string().min(1)),
    VITE_SUPABASE_ANON_KEY: requiredInProd(z.string().min(1)),
    VITE_POSTHOG_API_KEY: requiredInProd(z.string().min(1)),
    VITE_POSTHOG_HOST: z.string().default("https://us.i.posthog.com"),
    VITE_OTEL_EXPORTER_OTLP_ENDPOINT: z.url().optional(),
    VITE_OTEL_SAMPLE_RATE: z.coerce.number().int().positive().default(10),
    VITE_SENTRY_DSN: z.string().min(1).optional(),
    VITE_APP_VERSION: z.string().min(1).optional(),
    VITE_OUTLIT_PUBLIC_KEY: requiredInProd(z.string().min(1)),
  },

  runtimeEnv: { ...process.env, ...import.meta.env },
  emptyStringAsUndefined: true,
  skipValidation: isCI,
});
