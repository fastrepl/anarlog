import {
  AttachmentBackupGatewayError,
  createAttachmentBackupClient,
} from "@anlg/supabase/attachment-backups";
import { uploadPrivateAttachment } from "@anlg/supabase/storage";

import {
  cleanupAttachmentUploadCache,
  describeAttachmentUpload,
  prepareAttachmentUpload,
  readAttachmentUploadRange,
} from "@/db/client";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";

import {
  attachmentUploadRetryDelayMs,
  isPermanentNativeUploadMessage,
  isPrivateAttachmentIdentity,
} from "./upload-policy";
import {
  type MobileAttachmentUploadJob,
  mobileAttachmentUploadStore,
} from "./upload-store";

const MAX_JOBS_PER_PASS = 4;
const PASS_INTERVAL_MS = 20_000;

type AttachmentBackupClient = ReturnType<typeof createAttachmentBackupClient>;
type UploadStore = typeof mobileAttachmentUploadStore;

type RunnerDependencies = {
  client: AttachmentBackupClient;
  store: UploadStore;
  uploader: typeof uploadPrivateAttachment;
  supabaseUrl: string;
};

let requestActiveRunner: (() => void) | null = null;

export function requestMobileAttachmentUploads(): void {
  requestActiveRunner?.();
}

export function activateMobileAttachmentUploads(input: {
  accessToken: string;
}): {
  resume: () => void;
  pause: () => void;
  stop: () => void;
} {
  const dependencies: RunnerDependencies = {
    client: createAttachmentBackupClient({
      apiBaseUrl: env.apiUrl,
      getAccessToken: () => input.accessToken,
    }),
    store: mobileAttachmentUploadStore,
    uploader: uploadPrivateAttachment,
    supabaseUrl: env.supabaseUrl,
  };
  let enabled = false;
  let stopped = false;
  let running = false;
  let initialized = false;
  let runAgain = false;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  let controller: AbortController | undefined;

  const schedule = () => {
    clearTimeout(timeout);
    timeout = undefined;
    if (!enabled || stopped || running) return;
    timeout = setTimeout(request, PASS_INTERVAL_MS);
  };

  const tick = async () => {
    if (!enabled || stopped || running) return;
    running = true;
    clearTimeout(timeout);
    timeout = undefined;
    controller = new AbortController();
    try {
      if (!initialized) {
        await dependencies.store.resetInterrupted();
        initialized = true;
      }
      const processed = await runMobileAttachmentUploadPass(
        dependencies,
        controller.signal,
      );
      if (processed === MAX_JOBS_PER_PASS) runAgain = true;
    } catch (error) {
      if (!controller.signal.aborted) {
        captureOperationalError(error, {
          operation: "attachment_upload_pass",
          level: "warning",
        });
      }
    } finally {
      controller = undefined;
      running = false;
    }

    if (enabled && !stopped && runAgain) {
      runAgain = false;
      void tick();
    } else {
      schedule();
    }
  };

  function request() {
    if (!enabled || stopped) return;
    if (running) {
      runAgain = true;
      return;
    }
    void tick();
  }

  const pause = () => {
    enabled = false;
    runAgain = false;
    clearTimeout(timeout);
    timeout = undefined;
    controller?.abort(abortError("Attachment upload paused"));
  };
  const stop = () => {
    stopped = true;
    pause();
    if (requestActiveRunner === request) requestActiveRunner = null;
  };
  requestActiveRunner = request;

  return {
    resume: () => {
      if (stopped) return;
      enabled = true;
      request();
    },
    pause,
    stop,
  };
}

export async function runMobileAttachmentUploadPass(
  dependencies: RunnerDependencies,
  signal?: AbortSignal,
): Promise<number> {
  await dependencies.store.reconcile();
  let processed = 0;
  while (processed < MAX_JOBS_PER_PASS && !signal?.aborted) {
    const job = await dependencies.store.claimNext();
    if (!job) break;
    try {
      await runMobileAttachmentUpload(dependencies, job, signal);
    } catch (error) {
      await persistFailure(dependencies.store, job, error, signal?.aborted);
    }
    processed += 1;
  }
  return processed;
}

async function runMobileAttachmentUpload(
  dependencies: RunnerDependencies,
  job: MobileAttachmentUploadJob,
  signal?: AbortSignal,
): Promise<void> {
  throwIfAborted(signal);
  if (
    !job.cloudSyncEnabled ||
    job.attachmentDeleted ||
    job.localAvailability !== "present" ||
    !job.attachmentVersionMatches ||
    job.currentObjectKey
  ) {
    await dependencies.store.completeWithoutTransfer(job);
    if (job.cacheId) await cleanupCache(job, job.cacheId);
    return;
  }

  const descriptor = await describeAttachmentUpload(
    job.id,
    job.attemptCount,
    signal,
  );
  throwIfAborted(signal);
  const reservation = await dependencies.client.reserve(descriptor, signal);
  validateReservation(reservation, descriptor);
  await dependencies.store.setReservation(job, reservation);

  if (
    reservation.objectState === "current" ||
    reservation.objectState === "ready"
  ) {
    await requireCurrentVersion(dependencies.store, job);
    const objectKey =
      reservation.objectState === "current"
        ? reservation.objectKey
        : await promoteUpload(
            dependencies.client,
            descriptor.attachmentRef,
            descriptor.versionRef,
            reservation.objectKey,
            signal,
          );
    const completedCurrent = await dependencies.store.complete(job, objectKey);
    if (!completedCurrent) requestMobileAttachmentUploads();
    if (job.cacheId) await cleanupCache(job, job.cacheId);
    return;
  }

  let cacheId: string | undefined;
  try {
    const prepared = await prepareAttachmentUpload(
      {
        jobId: job.id,
        attemptCount: job.attemptCount,
        objectId: reservation.objectId,
        objectKey: reservation.objectKey,
      },
      signal,
    );
    cacheId = prepared.cacheId;
    if (prepared.ciphertextSizeBytes !== descriptor.ciphertextSizeBytes) {
      throw new PermanentAttachmentUploadError(
        "Prepared upload size did not match the reserved attachment",
      );
    }
    const grant = await dependencies.client.grantUpload(
      {
        objectKey: reservation.objectKey,
        ciphertextSha256: prepared.ciphertextSha256,
      },
      signal,
    );
    validateGrant(grant, reservation, prepared, descriptor.formatVersion);

    if (grant.uploadToken) {
      await dependencies.store.markPhase(job, "transferring");
      const upload = dependencies.uploader({
        objectKey: grant.objectKey,
        signedUploadToken: grant.uploadToken,
        ciphertextSha256: prepared.ciphertextSha256,
        ciphertextSizeBytes: prepared.ciphertextSizeBytes,
        supabaseUrl: dependencies.supabaseUrl,
        readRange: (start, end) =>
          readAttachmentUploadRange(
            {
              jobId: job.id,
              attemptCount: job.attemptCount,
              cacheId: prepared.cacheId,
              start,
              end,
            },
            signal,
          ),
      });
      const abortUpload = () => void upload.abort();
      signal?.addEventListener("abort", abortUpload, { once: true });
      try {
        await upload.promise;
      } finally {
        signal?.removeEventListener("abort", abortUpload);
      }
    }

    await requireCurrentVersion(dependencies.store, job);
    await dependencies.store.markPhase(job, "finalizing");
    await dependencies.client.finalize(reservation.objectKey, signal);
    const objectKey = await promoteUpload(
      dependencies.client,
      descriptor.attachmentRef,
      descriptor.versionRef,
      reservation.objectKey,
      signal,
    );
    const completedCurrent = await dependencies.store.complete(job, objectKey);
    if (!completedCurrent) requestMobileAttachmentUploads();
  } finally {
    if (cacheId) await cleanupCache(job, cacheId);
  }
}

async function promoteUpload(
  client: AttachmentBackupClient,
  attachmentRef: string,
  versionRef: string,
  candidateObjectKey: string,
  signal?: AbortSignal,
): Promise<string> {
  const current = await client.head(attachmentRef, signal);
  if (current?.versionRef === versionRef) return current.objectKey;

  try {
    const promoted = await client.promote(
      {
        objectKey: candidateObjectKey,
        expectedCurrentObjectKey: current?.objectKey ?? null,
      },
      signal,
    );
    if (promoted.currentVersionRef !== versionRef) {
      throw new PermanentAttachmentUploadError(
        "Promoted attachment version did not match the local recording",
      );
    }
    return promoted.currentObjectKey;
  } catch (error) {
    if (
      !(error instanceof AttachmentBackupGatewayError) ||
      error.status !== 409
    ) {
      throw error;
    }
    const raced = await client.head(attachmentRef, signal);
    if (raced?.versionRef === versionRef) return raced.objectKey;
    throw error;
  }
}

function validateReservation(
  reservation: Awaited<ReturnType<AttachmentBackupClient["reserve"]>>,
  descriptor: Awaited<ReturnType<typeof describeAttachmentUpload>>,
) {
  if (
    !isPrivateAttachmentIdentity(reservation.objectId, reservation.objectKey) ||
    !["reserved", "ready", "current"].includes(reservation.objectState) ||
    reservation.ciphertextSizeBytes !== descriptor.ciphertextSizeBytes ||
    reservation.formatVersion !== descriptor.formatVersion
  ) {
    throw new PermanentAttachmentUploadError(
      "Upload reservation did not match the local recording",
    );
  }
}

function validateGrant(
  grant: Awaited<ReturnType<AttachmentBackupClient["grantUpload"]>>,
  reservation: Awaited<ReturnType<AttachmentBackupClient["reserve"]>>,
  prepared: Awaited<ReturnType<typeof prepareAttachmentUpload>>,
  formatVersion: number,
) {
  if (
    grant.objectId !== reservation.objectId ||
    grant.objectKey !== reservation.objectKey ||
    grant.ciphertextSha256 !== prepared.ciphertextSha256 ||
    grant.ciphertextSizeBytes !== prepared.ciphertextSizeBytes ||
    grant.formatVersion !== formatVersion
  ) {
    throw new PermanentAttachmentUploadError(
      "Upload grant referenced a different attachment object",
    );
  }
}

async function requireCurrentVersion(
  store: UploadStore,
  job: MobileAttachmentUploadJob,
) {
  if (!(await store.isCurrentVersion(job))) {
    throw new PermanentAttachmentUploadError(
      "Local recording changed during upload",
    );
  }
}

async function persistFailure(
  store: UploadStore,
  job: MobileAttachmentUploadJob,
  error: unknown,
  aborted = false,
) {
  const message = error instanceof Error ? error.message : String(error);
  if (aborted) {
    await store.retry(job, "Attachment upload paused.", new Date());
    return;
  }
  if (isPermanentFailure(error)) {
    await store.fail(job, message);
    return;
  }
  const status =
    error instanceof AttachmentBackupGatewayError ? error.status : undefined;
  await store.retry(
    job,
    message,
    new Date(
      Date.now() + attachmentUploadRetryDelayMs(job.attemptCount, status),
    ),
  );
}

function isPermanentFailure(error: unknown): boolean {
  if (error instanceof PermanentAttachmentUploadError) return true;
  if (error instanceof AttachmentBackupGatewayError) {
    return [400, 404, 507].includes(error.status);
  }
  return (
    error instanceof Error && isPermanentNativeUploadMessage(error.message)
  );
}

async function cleanupCache(job: MobileAttachmentUploadJob, cacheId: string) {
  try {
    await cleanupAttachmentUploadCache({
      jobId: job.id,
      attemptCount: job.attemptCount,
      cacheId,
    });
  } catch (error) {
    captureOperationalError(error, {
      operation: "attachment_upload_cache_cleanup",
      level: "warning",
    });
  }
}

function throwIfAborted(signal?: AbortSignal) {
  if (!signal?.aborted) return;
  throw signal.reason ?? abortError("Attachment upload aborted");
}

function abortError(message: string) {
  const error = new Error(message);
  error.name = "AbortError";
  return error;
}

class PermanentAttachmentUploadError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PermanentAttachmentUploadError";
  }
}
