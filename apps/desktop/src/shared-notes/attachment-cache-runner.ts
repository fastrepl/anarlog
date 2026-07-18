import {
  type SharedAttachmentCacheJob,
  purgeForeignViewerSharedNoteCaches,
  sharedAttachmentCacheStore,
} from "./attachment-cache-store";
import type { createSharedAttachmentClient } from "./attachment-client";
import { SharedAttachmentGatewayError } from "./attachment-client";

import {
  attachmentTransferNative,
  NativeAttachmentTransferError,
} from "~/attachment-sync/native";

const MAX_JOBS_PER_PASS = 4;
const PASS_INTERVAL_MS = 20_000;
const MAX_RETRY_DELAY_MS = 15 * 60 * 1000;
const jobLocks = new Map<string, Promise<unknown>>();

type SharedAttachmentClient = ReturnType<typeof createSharedAttachmentClient>;

export type SharedAttachmentCacheRunnerDependencies = {
  viewerUserId: string;
  client: SharedAttachmentClient;
  store?: typeof sharedAttachmentCacheStore;
  native?: typeof attachmentTransferNative;
};

export async function runSharedAttachmentCachePass(
  dependencies: SharedAttachmentCacheRunnerDependencies,
  signal?: AbortSignal,
) {
  const store = dependencies.store ?? sharedAttachmentCacheStore;
  await store.recoverInterrupted(dependencies.viewerUserId);

  let processed = 0;
  while (processed < MAX_JOBS_PER_PASS && !signal?.aborted) {
    const job = await store.claimNext(dependencies.viewerUserId);
    if (!job) break;
    try {
      await runSharedAttachmentCacheJob(dependencies, job, signal);
    } catch (error) {
      if (
        job.availability === "downloading" &&
        error instanceof SharedAttachmentGatewayError &&
        [403, 404, 410].includes(error.status)
      ) {
        await store.markDeletePending(job);
      } else {
        const message = error instanceof Error ? error.message : String(error);
        await store.retry(
          job,
          signal?.aborted ? "Shared attachment transfer paused." : message,
          retryAt(job.attemptCount, error, signal?.aborted ?? false),
        );
      }
    }
    processed += 1;
  }
  return processed;
}

export async function runSharedAttachmentCacheJob(
  dependencies: SharedAttachmentCacheRunnerDependencies,
  job: SharedAttachmentCacheJob,
  signal?: AbortSignal,
) {
  return withJobLock(`${job.viewerUserId}:${job.attachmentId}`, () =>
    runSharedAttachmentCacheJobUnlocked(dependencies, job, signal),
  );
}

async function runSharedAttachmentCacheJobUnlocked(
  dependencies: SharedAttachmentCacheRunnerDependencies,
  job: SharedAttachmentCacheJob,
  signal?: AbortSignal,
) {
  throwIfAborted(signal);
  const store = dependencies.store ?? sharedAttachmentCacheStore;
  const native = dependencies.native ?? attachmentTransferNative;
  if (job.availability === "deleting") {
    await native.removeSharedAttachment(job.viewerUserId, job.attachmentId);
    await store.completeDelete(job);
    return;
  }

  const grant = await dependencies.client.download(
    job.shareId,
    job.attachmentId,
    signal,
  );
  if (
    grant.id !== job.attachmentId ||
    grant.filename !== job.filename ||
    grant.contentType !== job.contentType ||
    grant.sizeBytes !== job.sizeBytes ||
    grant.sha256 !== job.sha256
  ) {
    throw new PermanentSharedAttachmentError(
      "The download grant did not match the shared attachment manifest.",
    );
  }
  throwIfAborted(signal);
  const cached = await native.downloadSharedAttachment(
    {
      scopeId: job.viewerUserId,
      attachmentId: job.attachmentId,
      signedUrl: grant.signedUrl,
      expectedSha256: job.sha256,
      expectedSizeBytes: job.sizeBytes,
    },
    signal,
  );
  if (signal?.aborted) {
    try {
      await native.removeSharedAttachment(job.viewerUserId, job.attachmentId);
    } finally {
      throwIfAborted(signal);
    }
  }
  if (cached.sha256 !== job.sha256 || cached.sizeBytes !== job.sizeBytes) {
    throw new PermanentSharedAttachmentError(
      "The downloaded shared attachment failed integrity verification.",
    );
  }
  const accepted = await store.completeDownload(job, cached.cacheId);
  if (!accepted) {
    await native.removeSharedAttachment(job.viewerUserId, job.attachmentId);
  }
}

async function withJobLock<T>(key: string, operation: () => Promise<T>) {
  const previous = jobLocks.get(key) ?? Promise.resolve();
  const current = previous.catch(() => undefined).then(operation);
  jobLocks.set(key, current);
  try {
    return await current;
  } finally {
    if (jobLocks.get(key) === current) jobLocks.delete(key);
  }
}

export function startSharedAttachmentCacheRunner(
  dependencies: SharedAttachmentCacheRunnerDependencies,
) {
  const controller = new AbortController();
  let timeout: ReturnType<typeof setTimeout> | undefined;
  let initialized = false;

  const tick = async () => {
    if (controller.signal.aborted) return;
    try {
      if (!initialized) {
        const native = dependencies.native ?? attachmentTransferNative;
        await purgeForeignViewerSharedNoteCaches(
          dependencies.viewerUserId,
          (viewerUserId) => native.clearSharedAttachmentScope(viewerUserId),
          controller.signal,
        );
        initialized = true;
      }
      await runSharedAttachmentCachePass(dependencies, controller.signal);
    } catch (error) {
      if (!controller.signal.aborted) {
        console.error("[shared-attachments] cache pass failed", error);
      }
    }
    if (!controller.signal.aborted) {
      timeout = setTimeout(() => void tick(), PASS_INTERVAL_MS);
    }
  };
  void tick();

  return () => {
    controller.abort();
    if (timeout) clearTimeout(timeout);
  };
}

function retryAt(attemptCount: number, error: unknown, aborted: boolean) {
  const base =
    aborted ||
    (error instanceof SharedAttachmentGatewayError && error.status === 401)
      ? 30_000
      : isPermanentFailure(error)
        ? MAX_RETRY_DELAY_MS
        : 5_000;
  return new Date(
    Date.now() +
      Math.min(
        MAX_RETRY_DELAY_MS,
        base * 2 ** Math.min(Math.max(attemptCount - 1, 0), 8),
      ),
  );
}

function isPermanentFailure(error: unknown) {
  if (error instanceof PermanentSharedAttachmentError) return true;
  if (error instanceof NativeAttachmentTransferError) {
    return /checksum|integrity|invalid|mismatch|path|size/i.test(
      error.nativeMessage,
    );
  }
  return false;
}

function throwIfAborted(signal?: AbortSignal) {
  if (!signal?.aborted) return;
  if (signal.reason) throw signal.reason;
  const error = new Error("Shared attachment transfer aborted");
  error.name = "AbortError";
  throw error;
}

class PermanentSharedAttachmentError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PermanentSharedAttachmentError";
  }
}
