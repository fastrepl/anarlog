import { createEnv } from "@t3-oss/env-core";
import { z } from "zod";

export { env as coreEnv } from "@hypr/agent-core";

export const env = createEnv({
  server: {
    MODAL_TOKEN_ID: z.string(),
    MODAL_TOKEN_SECRET: z.string(),
    OPENROUTER_API_KEY: z.string(),
    DATABASE_URL: z.string(),
    MAGIC_PATTERNS_API_KEY: z.string().optional(),
    JINA_API_KEY: z.string().optional(),
    LANGSMITH_API_KEY: z.string().optional(),
    LANGSMITH_ORG_ID: z.string().optional(),
    LANGSMITH_PROJECT: z.string().optional().default("agent"),
    ANTHROPIC_API_KEY: z.string(),
  },
  runtimeEnv: process.env,
  emptyStringAsUndefined: true,
});
