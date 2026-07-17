import {
  commands as attachmentSyncCommands,
  type SharedAttachmentCacheResult,
} from "@hypr/plugin-attachment-sync";

export type { SharedAttachmentCacheResult };

export type UploadDescriptor = {
  attachmentRef: string;
  versionRef: string;
  ciphertextSizeBytes: number;
  formatVersion: number;
};

export type PreparedUpload = {
  cacheId: string;
  ciphertextSha256: string;
  ciphertextSizeBytes: number;
};

export type RestoredAttachment = {
  attachmentId: string;
  sessionId: string;
  relativePath: string;
  sizeBytes: number;
  sha256: string;
};

export const attachmentTransferNative = {
  describeUpload(jobId: string, attemptCount: number) {
    return unwrapNative(
      attachmentSyncCommands.describeUpload(jobId, attemptCount),
      "describe attachment upload",
    );
  },
  prepareUpload(
    jobId: string,
    attemptCount: number,
    objectId: string,
    objectKey: string,
  ) {
    return unwrapNative(
      attachmentSyncCommands.prepareUpload(
        jobId,
        attemptCount,
        objectId,
        objectKey,
      ),
      "prepare attachment upload",
    );
  },
  async readUploadRange(
    jobId: string,
    attemptCount: number,
    cacheId: string,
    start: number,
    end: number,
  ) {
    const bytes = await unwrapNative(
      attachmentSyncCommands.readUploadRange(
        jobId,
        attemptCount,
        cacheId,
        start,
        end,
      ),
      "read attachment upload cache",
    );
    return Uint8Array.from(bytes);
  },
  prepareSharedUpload(
    attachmentId: string,
    expectedSha256: string,
    expectedSizeBytes: number,
    expectedFilename: string,
    expectedContentType: string,
    expectedCloudObjectKey: string,
    signal?: AbortSignal,
  ) {
    return runCancellableNative(
      signal,
      "prepare shared attachment upload",
      {
        label: "shared attachment upload operation",
        begin: (operationId) =>
          attachmentSyncCommands.beginSharedUploadOperation(operationId),
        cancel: (operationId) =>
          attachmentSyncCommands.cancelSharedUploadOperation(operationId),
      },
      (operationId) =>
        attachmentSyncCommands.prepareSharedUpload(operationId, attachmentId, {
          sha256: expectedSha256,
          sizeBytes: expectedSizeBytes,
          filename: expectedFilename,
          contentType: expectedContentType,
          cloudObjectKey: expectedCloudObjectKey,
        }),
    );
  },
  async readSharedUploadRange(
    attachmentId: string,
    cacheId: string,
    expectedSha256: string,
    expectedSizeBytes: number,
    expectedFilename: string,
    expectedContentType: string,
    expectedCloudObjectKey: string,
    start: number,
    end: number,
  ) {
    const bytes = await unwrapNative(
      attachmentSyncCommands.readSharedUploadRange(
        attachmentId,
        cacheId,
        {
          sha256: expectedSha256,
          sizeBytes: expectedSizeBytes,
          filename: expectedFilename,
          contentType: expectedContentType,
          cloudObjectKey: expectedCloudObjectKey,
        },
        start,
        end,
      ),
      "read shared attachment upload snapshot",
    );
    return Uint8Array.from(bytes);
  },
  validateSharedUpload(
    attachmentId: string,
    cacheId: string,
    expectedSha256: string,
    expectedSizeBytes: number,
    expectedFilename: string,
    expectedContentType: string,
    expectedCloudObjectKey: string,
    signal?: AbortSignal,
  ) {
    return runCancellableNative(
      signal,
      "validate shared attachment upload",
      {
        label: "shared attachment upload operation",
        begin: (operationId) =>
          attachmentSyncCommands.beginSharedUploadOperation(operationId),
        cancel: (operationId) =>
          attachmentSyncCommands.cancelSharedUploadOperation(operationId),
      },
      (operationId) =>
        attachmentSyncCommands.validateSharedUpload(
          operationId,
          attachmentId,
          cacheId,
          {
            sha256: expectedSha256,
            sizeBytes: expectedSizeBytes,
            filename: expectedFilename,
            contentType: expectedContentType,
            cloudObjectKey: expectedCloudObjectKey,
          },
        ),
    );
  },
  cleanupSharedUpload(cacheId: string) {
    return unwrapNative(
      attachmentSyncCommands.cleanupSharedUpload(cacheId),
      "clean shared attachment upload snapshot",
    );
  },
  prepareDeleteGuard(
    jobId: string,
    attemptCount: number,
    signal?: AbortSignal,
  ) {
    return runCancellableNative(
      signal,
      "prepare attachment delete guard",
      {
        label: "attachment delete guard operation",
        begin: (operationId) =>
          attachmentSyncCommands.beginSharedUploadOperation(operationId),
        cancel: (operationId) =>
          attachmentSyncCommands.cancelSharedUploadOperation(operationId),
      },
      (operationId) =>
        attachmentSyncCommands.prepareDeleteGuard(
          operationId,
          jobId,
          attemptCount,
        ),
    );
  },
  commitDeleteGuard(
    jobId: string,
    attemptCount: number,
    guardId: string,
    signal?: AbortSignal,
  ) {
    return runCancellableNative(
      signal,
      "commit attachment delete guard",
      {
        label: "attachment delete guard operation",
        begin: (operationId) =>
          attachmentSyncCommands.beginSharedUploadOperation(operationId),
        cancel: (operationId) =>
          attachmentSyncCommands.cancelSharedUploadOperation(operationId),
      },
      (operationId) =>
        attachmentSyncCommands.commitDeleteGuard(
          operationId,
          jobId,
          attemptCount,
          guardId,
        ),
    );
  },
  reconcileDeleteGuards() {
    return unwrapNative(
      attachmentSyncCommands.reconcileDeleteGuards(),
      "reconcile attachment delete guards",
    );
  },
  downloadAndRestore(
    input: {
      jobId: string;
      attemptCount: number;
      objectId: string;
      signedUrl: string;
      ciphertextSha256: string;
      ciphertextSizeBytes: number;
      formatVersion: number;
    },
    signal?: AbortSignal,
  ) {
    return runCancellableDownload(
      null,
      signal,
      "restore attachment download",
      (operationId) =>
        attachmentSyncCommands.downloadAndRestore(
          operationId,
          input.jobId,
          input.attemptCount,
          input.objectId,
          input.signedUrl,
          input.ciphertextSha256,
          input.ciphertextSizeBytes,
          input.formatVersion,
        ),
    );
  },
  cleanupTransferCache(
    jobId: string,
    attemptCount: number,
    expectedCacheId: string,
  ) {
    return unwrapNative(
      attachmentSyncCommands.cleanupTransferCache(
        jobId,
        attemptCount,
        expectedCacheId,
      ),
      "clean attachment transfer cache",
    );
  },
  downloadSharedAttachment(
    input: {
      scopeId: string;
      attachmentId: string;
      signedUrl: string;
      expectedSha256: string;
      expectedSizeBytes: number;
    },
    signal?: AbortSignal,
  ) {
    return runCancellableDownload<SharedAttachmentCacheResult>(
      input.scopeId,
      signal,
      "download shared attachment",
      (operationId) =>
        attachmentSyncCommands.downloadSharedAttachment(
          operationId,
          input.scopeId,
          input.attachmentId,
          input.signedUrl,
          input.expectedSha256,
          input.expectedSizeBytes,
        ),
    );
  },
  sharedAttachmentPath(scopeId: string, attachmentId: string) {
    return unwrapNative(
      attachmentSyncCommands.sharedAttachmentPath(scopeId, attachmentId),
      "resolve shared attachment cache",
    );
  },
  removeSharedAttachment(scopeId: string, attachmentId: string) {
    return unwrapNative(
      attachmentSyncCommands.removeSharedAttachment(scopeId, attachmentId),
      "remove shared attachment cache",
    );
  },
  clearSharedAttachmentScope(scopeId: string) {
    return unwrapNative(
      attachmentSyncCommands.clearSharedAttachmentScope(scopeId),
      "clear shared attachment cache",
    );
  },
};

async function runCancellableDownload<T>(
  scopeId: string | null,
  signal: AbortSignal | undefined,
  label: string,
  operation: (
    operationId: string,
  ) => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
) {
  return runCancellableNative(
    signal,
    label,
    {
      label: "attachment download",
      begin: (operationId) =>
        attachmentSyncCommands.beginAttachmentDownload(operationId, scopeId),
      cancel: (operationId) =>
        attachmentSyncCommands.cancelAttachmentDownload(operationId),
    },
    operation,
  );
}

async function runCancellableNative<T>(
  signal: AbortSignal | undefined,
  label: string,
  control: {
    label: string;
    begin: (
      operationId: string,
    ) => Promise<
      { status: "ok"; data: unknown } | { status: "error"; error: string }
    >;
    cancel: (
      operationId: string,
    ) => Promise<
      { status: "ok"; data: boolean } | { status: "error"; error: string }
    >;
  },
  operation: (
    operationId: string,
  ) => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
) {
  throwIfAborted(signal);
  const operationId = crypto.randomUUID();
  let begun = false;
  let abortRequested = false;
  let cancellation: Promise<boolean> | undefined;
  const cancel = () => {
    cancellation ??= unwrapNative(
      control.cancel(operationId),
      `cancel ${control.label}`,
    ).catch(() => false);
    return cancellation;
  };
  const abort = () => {
    abortRequested = true;
    if (begun) void cancel();
  };
  signal?.addEventListener("abort", abort, { once: true });

  try {
    await unwrapNative(control.begin(operationId), `begin ${control.label}`);
    begun = true;
    if (abortRequested || signal?.aborted) {
      await cancel();
      throwAbort(signal);
    }
    return await unwrapNative(operation(operationId), label);
  } finally {
    signal?.removeEventListener("abort", abort);
    if (begun) await cancel();
  }
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) throwAbort(signal);
}

function throwAbort(signal?: AbortSignal): never {
  if (signal?.reason) throw signal.reason;
  const error = new Error("Attachment transfer aborted");
  error.name = "AbortError";
  throw error;
}

async function unwrapNative<T>(
  operation: Promise<
    { status: "ok"; data: T } | { status: "error"; error: string }
  >,
  label: string,
): Promise<T> {
  const result = await operation;
  if (result.status === "error") {
    throw new NativeAttachmentTransferError(label, result.error);
  }
  return result.data;
}

export class NativeAttachmentTransferError extends Error {
  constructor(
    label: string,
    readonly nativeMessage: string,
  ) {
    super(`${label} failed: ${nativeMessage}`);
    this.name = "NativeAttachmentTransferError";
  }
}
