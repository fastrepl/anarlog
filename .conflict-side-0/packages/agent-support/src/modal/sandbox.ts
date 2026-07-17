import type { App, Image } from "modal";

import { env } from "../env";
import { getModalClient } from "./client";

const APP_NAME = "hypr-slack-internal";
const DEFAULT_TIMEOUT_MS = 5 * 60 * 1000;
const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 1000;
export const REPO_PATH = "/root/hyprnote";

export type BunSandbox = Awaited<ReturnType<typeof createBunSandbox>>;

let cachedApp: App | null = null;

async function getAppAndImage(): Promise<{
  app: App;
  image: Image;
}> {
  const modal = getModalClient();

  if (!cachedApp) {
    cachedApp = await modal.apps.fromName(APP_NAME, {
      createIfMissing: true,
    });
  }

  const image = modal.images
    .fromRegistry("oven/bun:1.3-debian")
    .dockerfileCommands([
      "RUN apt-get update && apt-get install -y curl git bash npm && rm -rf /var/lib/apt/lists/*",
      "RUN npm install -g @anthropic-ai/claude-code",
      "WORKDIR /app",
      "RUN bun add stripe @supabase/supabase-js loops pg posthog-node",
      `RUN git clone --depth 1 https://github.com/fastrepl/hyprnote.git ${REPO_PATH}`,
    ]);

  return { app: cachedApp, image };
}

async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isRetryableError(error: unknown): boolean {
  if (error instanceof Error) {
    const message = error.message.toLowerCase();
    return (
      message.includes("rst_stream") ||
      message.includes("internal") ||
      message.includes("unavailable") ||
      message.includes("deadline") ||
      message.includes("connection")
    );
  }
  return false;
}

async function withRetry<T>(
  fn: () => Promise<T>,
  options: {
    maxRetries: number;
    delayMs: number;
    shouldRetry: (error: unknown) => boolean;
    onRetry?: (attempt: number, error: unknown) => void;
  },
): Promise<T> {
  let lastError: unknown;

  for (let attempt = 1; attempt <= options.maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;
      if (attempt < options.maxRetries && options.shouldRetry(error)) {
        options.onRetry?.(attempt, error);
        await sleep(options.delayMs * attempt);
      } else {
        throw error;
      }
    }
  }

  throw lastError;
}

class SandboxManager {
  async create(options?: CreateBunSandboxOptions): Promise<BunSandbox> {
    return await createBunSandbox(options);
  }

  async release(sandbox: BunSandbox): Promise<void> {
    await terminateSandbox(sandbox);
  }
}

export const sandboxManager = new SandboxManager();

interface CreateBunSandboxOptions {
  timeoutMs?: number;
}

export async function createBunSandbox(options?: CreateBunSandboxOptions) {
  return withRetry(() => createBunSandboxInternal(options), {
    maxRetries: MAX_RETRIES,
    delayMs: RETRY_DELAY_MS,
    shouldRetry: isRetryableError,
    onRetry: (attempt, error) => {
      console.warn(
        `Sandbox creation failed (attempt ${attempt}/${MAX_RETRIES}), retrying...`,
        error instanceof Error ? error.message : error,
      );
    },
  });
}

async function createBunSandboxInternal(options?: CreateBunSandboxOptions) {
  const { app, image } = await getAppAndImage();
  const modal = getModalClient();

  const sandbox = await modal.sandboxes.create(app, image, {
    verbose: true,
    timeoutMs: options?.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    env: {
      STRIPE_SECRET_KEY: env.STRIPE_SECRET_KEY,
      SUPABASE_URL: env.SUPABASE_URL,
      SUPABASE_SERVICE_ROLE_KEY: env.SUPABASE_SERVICE_ROLE_KEY,
      DATABASE_URL: env.DATABASE_URL,
      ANTHROPIC_API_KEY: env.ANTHROPIC_API_KEY,
      ...(env.LOOPS_API_KEY && { LOOPS_API_KEY: env.LOOPS_API_KEY }),
      ...(env.POSTHOG_API_KEY && { POSTHOG_API_KEY: env.POSTHOG_API_KEY }),
      ...(env.POSTHOG_HOST && { POSTHOG_HOST: env.POSTHOG_HOST }),
    },
  });

  return sandbox;
}

export async function terminateSandbox(sandbox: BunSandbox) {
  try {
    await sandbox.terminate();
  } catch (error) {
    console.error("Failed to terminate sandbox:", error);
  }
}

export interface SandboxRunResult<T> {
  success: boolean;
  data: T;
  executionTimeMs: number;
}

export async function runInSandbox<T>(
  options: CreateBunSandboxOptions | undefined,
  fn: (sandbox: BunSandbox) => Promise<{ success: boolean; data: T }>,
): Promise<SandboxRunResult<T>> {
  const startTime = Date.now();
  let sandbox: BunSandbox | null = null;

  try {
    sandbox = await createBunSandbox(options);
    const result = await fn(sandbox);
    return {
      success: result.success,
      data: result.data,
      executionTimeMs: Date.now() - startTime,
    };
  } catch (error) {
    throw Object.assign(
      error instanceof Error ? error : new Error(String(error)),
      {
        executionTimeMs: Date.now() - startTime,
      },
    );
  } finally {
    if (sandbox) {
      await terminateSandbox(sandbox);
    }
  }
}
