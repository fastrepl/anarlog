import { createEnv } from "@t3-oss/env-core";
import { z } from "zod";

export const env = createEnv({
  server: {
    PORT: z.coerce.number().default(8788),
    NODE_ENV: z
      .enum(["development", "test", "production"])
      .default("development"),
    DATABASE_URL: z.string().min(1),
    SUPABASE_URL: z.url(),
    SUPABASE_SERVICE_ROLE_KEY: z.string().min(1),
    STRIPE_SECRET_KEY: z.string().min(1),
    STRIPE_WEBHOOK_SECRET: z.string().min(1),
    POSTHOG_API_KEY: z.string().min(1).optional(),
    LOOPS_API_KEY: z.string().min(1).optional(),
    LOOPS_TRIAL_ENDING_TRANSACTIONAL_ID: z.string().min(1).optional(),
  },
  runtimeEnv: Bun.env,
  emptyStringAsUndefined: true,
  skipValidation: Bun.env.CI === "true",
});
