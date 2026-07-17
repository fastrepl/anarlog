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
  describeUpload(jobId: string) {
    return unwrapNative(
      attachmentSyncCommands.describeUpload(jobId),
      "describe attachment upload",
    );
  },
  prepareUpload(jobId: string, objectId: string, objectKey: string) {
    return unwrapNative(
      attachmentSyncCommands.prepareUpload(jobId, objectId, objectKey),
      "prepare attachment upload",
    );
  },
  async readUploadRange(jobId: string, start: number, end: number) {
    const bytes = await unwrapNative(
      attachmentSyncCommands.readUploadRange(jobId, start, end),
      "read attachment upload cache",
    );
    return Uint8Array.from(bytes);
  },
  async readAttachmentRange(attachmentId: string, start: number, end: number) {
    const bytes = await unwrapNative(
      attachmentSyncCommands.readAttachmentRange(attachmentId, start, end),
      "read local attachment",
    );
    return Uint8Array.from(bytes);
  },
  downloadAndRestore(input: {
    jobId: string;
    objectId: string;
    signedUrl: string;
    supabaseUrl: string;
    ciphertextSha256: string;
    ciphertextSizeBytes: number;
    formatVersion: number;
  }) {
    return unwrapNative(
      attachmentSyncCommands.downloadAndRestore(
        input.jobId,
        input.objectId,
        input.signedUrl,
        input.supabaseUrl,
        input.ciphertextSha256,
        input.ciphertextSizeBytes,
        input.formatVersion,
      ),
      "restore attachment download",
    );
  },
  cleanupTransferCache(jobId: string) {
    return unwrapNative(
      attachmentSyncCommands.cleanupTransferCache(jobId),
      "clean attachment transfer cache",
    );
  },
  downloadSharedAttachment(input: {
    scopeId: string;
    attachmentId: string;
    signedUrl: string;
    supabaseUrl: string;
    expectedSha256: string;
    expectedSizeBytes: number;
  }) {
    return unwrapNative<SharedAttachmentCacheResult>(
      attachmentSyncCommands.downloadSharedAttachment(
        input.scopeId,
        input.attachmentId,
        input.signedUrl,
        input.supabaseUrl,
        input.expectedSha256,
        input.expectedSizeBytes,
      ),
      "download shared attachment",
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
