import { uploadPrivateAttachment } from "@hypr/supabase/storage";

import {
  AttachmentBackupGatewayError,
  createAttachmentBackupClient,
} from "./client";
import {
  attachmentTransferNative,
  NativeAttachmentTransferError,
} from "./native";
import { type AttachmentTransferJob, attachmentTransferStore } from "./store";

const MAX_JOBS_PER_PASS = 4;
const PASS_INTERVAL_MS = 20_000;
const MAX_RETRY_DELAY_MS = 15 * 60 * 1000;

type AttachmentBackupClient = ReturnType<typeof createAttachmentBackupClient>;

export type AttachmentTransferRunnerDependencies = {
  client: AttachmentBackupClient;
  supabaseUrl: string;
  store?: typeof attachmentTransferStore;
  native?: typeof attachmentTransferNative;
  uploader?: typeof uploadPrivateAttachment;
};

export async function runAttachmentTransferPass(
  dependencies: AttachmentTransferRunnerDependencies,
  signal?: AbortSignal,
): Promise<number> {
  const store = dependencies.store ?? attachmentTransferStore;
  await store.recoverInterrupted();
  await store.reconcile();

  let processed = 0;
  while (processed < MAX_JOBS_PER_PASS && !signal?.aborted) {
    const job = await store.claimNext();
    if (!job) break;

    try {
      await runAttachmentTransferJob(dependencies, job, signal);
    } catch (error) {
      await persistTransferFailure(store, job, error, signal?.aborted ?? false);
    }
    processed += 1;
  }
  return processed;
}

export async function runAttachmentTransferJob(
  dependencies: AttachmentTransferRunnerDependencies,
  job: AttachmentTransferJob,
  signal?: AbortSignal,
): Promise<void> {
  throwIfAborted(signal);
  switch (job.direction) {
    case "upload":
      await runUpload(dependencies, job, signal);
      return;
    case "download":
      await runDownload(dependencies, job, signal);
      return;
    case "delete":
      await runDelete(dependencies, job, signal);
      return;
  }
}

export function startAttachmentTransferRunner(
  dependencies: AttachmentTransferRunnerDependencies,
  externalSignal?: AbortSignal,
): () => void {
  const controller = new AbortController();
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const stop = () => {
    controller.abort();
    if (timeout) clearTimeout(timeout);
  };
  const stopFromExternalSignal = () => stop();
  externalSignal?.addEventListener("abort", stopFromExternalSignal, {
    once: true,
  });

  const tick = async () => {
    if (controller.signal.aborted) return;
    try {
      await runAttachmentTransferPass(dependencies, controller.signal);
    } catch (error) {
      if (!controller.signal.aborted) {
        console.error("[attachment-sync] transfer pass failed", error);
      }
    }
    if (!controller.signal.aborted) {
      timeout = setTimeout(() => void tick(), PASS_INTERVAL_MS);
    }
  };
  void tick();

  return () => {
    externalSignal?.removeEventListener("abort", stopFromExternalSignal);
    stop();
  };
}

async function runUpload(
  dependencies: AttachmentTransferRunnerDependencies,
  job: AttachmentTransferJob,
  signal?: AbortSignal,
) {
  const store = dependencies.store ?? attachmentTransferStore;
  const native = dependencies.native ?? attachmentTransferNative;
  const uploader = dependencies.uploader ?? uploadPrivateAttachment;
  if (
    !job.cloudSyncEnabled ||
    job.attachmentDeleted ||
    job.localAvailability !== "present" ||
    !job.attachmentVersionMatches
  ) {
    await store.completeWithoutTransfer(job.id);
    return;
  }

  const descriptor = await native.describeUpload(job.id);
  throwIfAborted(signal);
  const reservation = await dependencies.client.reserve(
    {
      attachmentRef: descriptor.attachmentRef,
      versionRef: descriptor.versionRef,
      ciphertextSizeBytes: descriptor.ciphertextSizeBytes,
      formatVersion: descriptor.formatVersion,
    },
    signal,
  );
  await store.setUploadReservation(job.id, reservation);

  if (
    reservation.objectState === "current" ||
    reservation.objectState === "ready"
  ) {
    try {
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
      const accepted = await store.completeUpload(job, objectKey);
      if (!accepted) {
        await dependencies.client.delete(objectKey, signal);
      }
    } finally {
      if (!signal?.aborted) {
        await cleanupTransferCache(native, job.id);
      }
    }
    return;
  }

  let shouldCleanCache = false;
  try {
    const prepared = await native.prepareUpload(
      job.id,
      reservation.objectId,
      reservation.objectKey,
    );
    shouldCleanCache = true;
    throwIfAborted(signal);

    const grant = await dependencies.client.grantUpload(
      {
        objectKey: reservation.objectKey,
        ciphertextSha256: prepared.ciphertextSha256,
      },
      signal,
    );
    if (
      grant.objectId !== reservation.objectId ||
      grant.objectKey !== reservation.objectKey ||
      grant.ciphertextSha256 !== prepared.ciphertextSha256 ||
      grant.ciphertextSizeBytes !== prepared.ciphertextSizeBytes ||
      grant.formatVersion !== descriptor.formatVersion
    ) {
      throw new PermanentAttachmentTransferError(
        "The upload grant referenced a different attachment object.",
      );
    }

    if (grant.uploadToken) {
      await store.markPhase(job.id, "transferring");
      const upload = uploader({
        objectKey: grant.objectKey,
        signedUploadToken: grant.uploadToken,
        ciphertextSha256: prepared.ciphertextSha256,
        ciphertextSizeBytes: prepared.ciphertextSizeBytes,
        supabaseUrl: dependencies.supabaseUrl,
        readRange: (start, end) => native.readUploadRange(job.id, start, end),
      });
      const abortUpload = () => void upload.abort();
      signal?.addEventListener("abort", abortUpload, { once: true });
      try {
        await upload.promise;
      } finally {
        signal?.removeEventListener("abort", abortUpload);
      }
    }

    await store.markPhase(job.id, "finalizing");
    await dependencies.client.finalize(reservation.objectKey, signal);
    const objectKey = await promoteUpload(
      dependencies.client,
      descriptor.attachmentRef,
      descriptor.versionRef,
      reservation.objectKey,
      signal,
    );
    const accepted = await store.completeUpload(job, objectKey);
    if (!accepted) {
      await dependencies.client.delete(objectKey, signal);
    }
    shouldCleanCache = true;
  } finally {
    if (shouldCleanCache && !signal?.aborted) {
      await cleanupTransferCache(native, job.id);
    }
  }
}

async function promoteUpload(
  client: AttachmentBackupClient,
  attachmentRef: string,
  versionRef: string,
  candidateObjectKey: string,
  signal?: AbortSignal,
) {
  const current = await client.head(attachmentRef, signal);
  if (current?.versionRef === versionRef) {
    return current.objectKey;
  }

  try {
    const promoted = await client.promote(
      {
        objectKey: candidateObjectKey,
        expectedCurrentObjectKey: current?.objectKey ?? null,
      },
      signal,
    );
    if (promoted.currentVersionRef !== versionRef) {
      throw new PermanentAttachmentTransferError(
        "The promoted attachment version did not match the local attachment.",
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

async function runDownload(
  dependencies: AttachmentTransferRunnerDependencies,
  job: AttachmentTransferJob,
  signal?: AbortSignal,
) {
  const store = dependencies.store ?? attachmentTransferStore;
  const native = dependencies.native ?? attachmentTransferNative;
  if (
    !job.cloudSyncEnabled ||
    job.attachmentDeleted ||
    job.localAvailability === "present" ||
    !job.attachmentVersionMatches ||
    job.currentObjectKey !== job.objectKey
  ) {
    await store.completeWithoutTransfer(job.id);
    return;
  }

  const download = await dependencies.client.download(job.objectKey, signal);
  if (download.objectKey !== job.objectKey) {
    throw new PermanentAttachmentTransferError(
      "The download grant referenced a different attachment object.",
    );
  }
  await store.setDownloadGrant(job.id, download);
  const restored = await native.downloadAndRestore({
    jobId: job.id,
    objectId: download.objectId,
    signedUrl: download.signedUrl,
    supabaseUrl: dependencies.supabaseUrl,
    ciphertextSha256: download.ciphertextSha256,
    ciphertextSizeBytes: download.ciphertextSizeBytes,
    formatVersion: download.formatVersion,
  });
  if (
    restored.attachmentId !== job.attachmentId ||
    restored.sessionId !== job.sessionId ||
    restored.sha256 !== job.expectedSha256 ||
    restored.sizeBytes !== job.expectedSizeBytes
  ) {
    throw new PermanentAttachmentTransferError(
      "The restored attachment did not match the requested attachment.",
    );
  }
  await cleanupTransferCache(native, job.id);
}

async function runDelete(
  dependencies: AttachmentTransferRunnerDependencies,
  job: AttachmentTransferJob,
  signal?: AbortSignal,
) {
  const store = dependencies.store ?? attachmentTransferStore;
  if (
    job.cloudSyncEnabled &&
    !job.attachmentDeleted &&
    job.currentObjectKey === job.objectKey
  ) {
    await store.completeWithoutTransfer(job.id);
    return;
  }

  await dependencies.client.delete(job.objectKey, signal);
  await store.completeDelete(job);
}

async function persistTransferFailure(
  store: typeof attachmentTransferStore,
  job: AttachmentTransferJob,
  error: unknown,
  aborted: boolean,
) {
  const message = error instanceof Error ? error.message : String(error);
  if (aborted) {
    await store.retry(job, "Attachment transfer paused.", new Date());
  } else if (isPermanentFailure(error)) {
    await store.fail(job, message);
  } else {
    await store.retry(job, message, retryAt(job.attemptCount, error));
  }
}

function isPermanentFailure(error: unknown) {
  if (error instanceof PermanentAttachmentTransferError) return true;
  if (error instanceof AttachmentBackupGatewayError) {
    return [400, 404, 507].includes(error.status);
  }
  if (error instanceof NativeAttachmentTransferError) {
    return /checksum|cipher|integrity|format|invalid|mismatch|path|source/i.test(
      error.nativeMessage,
    );
  }
  return false;
}

function retryAt(attemptCount: number, error: unknown) {
  const base =
    error instanceof AttachmentBackupGatewayError && error.status === 403
      ? 5 * 60 * 1000
      : 5_000;
  const delay = Math.min(
    MAX_RETRY_DELAY_MS,
    base * 2 ** Math.min(Math.max(attemptCount - 1, 0), 8),
  );
  return new Date(Date.now() + delay);
}

function throwIfAborted(signal?: AbortSignal) {
  if (!signal?.aborted) return;
  if (signal.reason) throw signal.reason;
  const error = new Error("Attachment transfer aborted");
  error.name = "AbortError";
  throw error;
}

class PermanentAttachmentTransferError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PermanentAttachmentTransferError";
  }
}

async function cleanupTransferCache(
  native: typeof attachmentTransferNative,
  jobId: string,
) {
  try {
    await native.cleanupTransferCache(jobId);
  } catch (error) {
    console.error("[attachment-sync] failed to clean transfer cache", error);
  }
}
