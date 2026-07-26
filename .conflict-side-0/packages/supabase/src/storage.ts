import { Upload } from "tus-js-client";

export const STORAGE_CONFIG = {
  bucketName: "audio-files",
  chunkSize: 6 * 1024 * 1024,
  retryDelays: [0, 3000, 5000, 10000, 20000],
} as const;

export const PRIVATE_ATTACHMENT_STORAGE_CONFIG = {
  bucketName: "attachment-backups",
  contentType: "application/octet-stream",
  chunkSize: 6 * 1024 * 1024,
  maxCiphertextSizeBytes: 520 * 1024 * 1024,
  retryDelays: [0, 3000, 5000, 10000, 20000],
} as const;

export const SHARED_ATTACHMENT_STORAGE_CONFIG = {
  bucketName: "shared-note-attachments",
  chunkSize: 6 * 1024 * 1024,
  maxSizeBytes: 512 * 1024 * 1024,
  retryDelays: [0, 3000, 5000, 10000, 20000],
} as const;

const PRIVATE_ATTACHMENT_OBJECT_KEY_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$/;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const UUID_V4_PATTERN =
  "[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}";
const SHARED_ATTACHMENT_OBJECT_KEY_PATTERN = new RegExp(
  `^${UUID_V4_PATTERN}/${UUID_V4_PATTERN}/${UUID_V4_PATTERN}\\.sna1$`,
);

export function getTusEndpoint(supabaseUrl: string): string {
  const parsed = new URL(supabaseUrl);
  const isHostedSupabase = parsed.hostname.endsWith(".supabase.co");

  if (isHostedSupabase) {
    const projectId = parsed.hostname.split(".")[0];
    return `https://${projectId}.storage.supabase.co/storage/v1/upload/resumable`;
  }

  return new URL("/storage/v1/upload/resumable", parsed.origin).toString();
}

function getSignedTusEndpoint(supabaseUrl: string): string {
  return `${getTusEndpoint(supabaseUrl)}/sign`;
}

export function buildObjectName(userId: string, fileName: string): string {
  return `${userId}/${Date.now()}-${fileName}`;
}

export function uploadAudio(options: {
  file: File | Blob;
  fileName: string;
  contentType: string;
  supabaseUrl: string;
  accessToken: string;
  userId: string;
  onProgress?: (percentage: number) => void;
}): { promise: Promise<string>; abort: () => void } {
  const objectName = buildObjectName(options.userId, options.fileName);
  const endpoint = getTusEndpoint(options.supabaseUrl);

  let upload: Upload | null = null;

  const promise = new Promise<string>((resolve, reject) => {
    upload = new Upload(options.file, {
      endpoint,
      retryDelays: [...STORAGE_CONFIG.retryDelays],
      headers: {
        authorization: `Bearer ${options.accessToken}`,
        "x-upsert": "true",
      },
      uploadDataDuringCreation: true,
      removeFingerprintOnSuccess: true,
      metadata: {
        bucketName: STORAGE_CONFIG.bucketName,
        objectName,
        contentType: options.contentType,
        cacheControl: "3600",
      },
      chunkSize: STORAGE_CONFIG.chunkSize,
      onError: (error) => {
        reject(error);
      },
      onProgress: (bytesUploaded, bytesTotal) => {
        if (options.onProgress && bytesTotal > 0) {
          options.onProgress((bytesUploaded / bytesTotal) * 100);
        }
      },
      onSuccess: () => {
        resolve(objectName);
      },
    });

    upload.findPreviousUploads().then((previousUploads) => {
      if (previousUploads.length > 0) {
        upload!.resumeFromPreviousUpload(previousUploads[0]);
      }
      upload!.start();
    });
  });

  return {
    promise,
    abort: () => {
      upload?.abort();
    },
  };
}

export function uploadPrivateAttachment(options: {
  objectKey: string;
  signedUploadToken: string;
  ciphertextSha256: string;
  ciphertextSizeBytes: number;
  supabaseUrl: string;
  readRange: (start: number, end: number) => Promise<Uint8Array>;
  onProgress?: (percentage: number) => void;
}): { promise: Promise<string>; abort: () => Promise<void> } {
  if (!PRIVATE_ATTACHMENT_OBJECT_KEY_PATTERN.test(options.objectKey)) {
    throw new Error("invalid private attachment object key");
  }
  if (
    options.signedUploadToken.length === 0 ||
    options.signedUploadToken.length > 8192 ||
    /[\u0000-\u001f\u007f]/.test(options.signedUploadToken)
  ) {
    throw new Error("invalid private attachment upload token");
  }
  if (!SHA256_PATTERN.test(options.ciphertextSha256)) {
    throw new Error("invalid private attachment checksum");
  }
  if (
    !Number.isSafeInteger(options.ciphertextSizeBytes) ||
    options.ciphertextSizeBytes <= 0 ||
    options.ciphertextSizeBytes >
      PRIVATE_ATTACHMENT_STORAGE_CONFIG.maxCiphertextSizeBytes
  ) {
    throw new Error("invalid private attachment size");
  }

  const endpoint = getSignedTusEndpoint(options.supabaseUrl);
  const fingerprint = [
    "anarlog-private-attachment-v1",
    options.objectKey,
    options.ciphertextSizeBytes,
    options.ciphertextSha256,
  ].join(":");
  let upload: Upload | null = null;
  let cancelled = false;
  let settled = false;
  let rejectTransfer = (_error: Error) => {};

  const promise = new Promise<string>((resolve, reject) => {
    const settleResolve = (value: string) => {
      if (!settled) {
        settled = true;
        resolve(value);
      }
    };
    const settleReject = (error: Error) => {
      if (!settled) {
        settled = true;
        reject(error);
      }
    };
    rejectTransfer = settleReject;

    upload = new Upload(new Blob([]), {
      endpoint,
      retryDelays: [...PRIVATE_ATTACHMENT_STORAGE_CONFIG.retryDelays],
      headers: {
        "x-signature": options.signedUploadToken,
      },
      uploadSize: options.ciphertextSizeBytes,
      uploadDataDuringCreation: true,
      removeFingerprintOnSuccess: true,
      fingerprint: async () => fingerprint,
      fileReader: {
        openFile: async () => ({
          size: options.ciphertextSizeBytes,
          slice: async (start: number, end: number) => {
            if (
              !Number.isSafeInteger(start) ||
              !Number.isSafeInteger(end) ||
              start < 0 ||
              end <= start ||
              end > options.ciphertextSizeBytes ||
              end - start > PRIVATE_ATTACHMENT_STORAGE_CONFIG.chunkSize
            ) {
              throw new Error("invalid private attachment read range");
            }

            const bytes = await options.readRange(start, end);
            if (
              !(bytes instanceof Uint8Array) ||
              bytes.byteLength !== end - start
            ) {
              throw new Error("private attachment read was incomplete");
            }

            return {
              value: new Blob([bytes]),
              done: end === options.ciphertextSizeBytes,
            };
          },
          close: () => {},
        }),
      },
      metadata: {
        bucketName: PRIVATE_ATTACHMENT_STORAGE_CONFIG.bucketName,
        objectName: options.objectKey,
        contentType: PRIVATE_ATTACHMENT_STORAGE_CONFIG.contentType,
        cacheControl: "0",
        metadata: JSON.stringify({
          formatVersion: 1,
          ciphertextSha256: options.ciphertextSha256,
        }),
      },
      chunkSize: PRIVATE_ATTACHMENT_STORAGE_CONFIG.chunkSize,
      onError: settleReject,
      onProgress: (bytesUploaded, bytesTotal) => {
        if (options.onProgress && bytesTotal > 0) {
          options.onProgress((bytesUploaded / bytesTotal) * 100);
        }
      },
      onSuccess: () => settleResolve(options.objectKey),
    });

    upload
      .findPreviousUploads()
      .then((previousUploads) => {
        if (cancelled) {
          return;
        }
        const previous = previousUploads.find(
          (candidate) => candidate.size === options.ciphertextSizeBytes,
        );
        if (previous) {
          upload!.resumeFromPreviousUpload(previous);
        }
        if (cancelled) {
          return;
        }
        upload!.start();
      })
      .catch(settleReject);
  });

  return {
    promise,
    abort: async () => {
      if (cancelled || settled) {
        return;
      }
      cancelled = true;
      const error = new Error("private attachment upload aborted");
      error.name = "AbortError";
      rejectTransfer(error);
      await upload?.abort();
    },
  };
}

export function uploadSharedAttachment(options: {
  objectKey: string;
  signedUploadToken: string;
  contentType: string;
  sha256: string;
  sizeBytes: number;
  supabaseUrl: string;
  readRange: (start: number, end: number) => Promise<Uint8Array>;
  onProgress?: (percentage: number) => void;
}): { promise: Promise<string>; abort: () => Promise<void> } {
  if (!SHARED_ATTACHMENT_OBJECT_KEY_PATTERN.test(options.objectKey)) {
    throw new Error("invalid shared attachment object key");
  }
  if (
    options.signedUploadToken.length === 0 ||
    options.signedUploadToken.length > 8192 ||
    /[\u0000-\u001f\u007f]/.test(options.signedUploadToken)
  ) {
    throw new Error("invalid shared attachment upload token");
  }
  if (!SHA256_PATTERN.test(options.sha256)) {
    throw new Error("invalid shared attachment checksum");
  }
  if (
    !Number.isSafeInteger(options.sizeBytes) ||
    options.sizeBytes <= 0 ||
    options.sizeBytes > SHARED_ATTACHMENT_STORAGE_CONFIG.maxSizeBytes
  ) {
    throw new Error("invalid shared attachment size");
  }
  if (!isSafeSharedContentType(options.contentType)) {
    throw new Error("invalid shared attachment content type");
  }

  const endpoint = getSignedTusEndpoint(options.supabaseUrl);
  const fingerprint = [
    "anarlog-shared-attachment-v1",
    options.objectKey,
    options.sizeBytes,
    options.sha256,
  ].join(":");
  let upload: Upload | null = null;
  let cancelled = false;
  let settled = false;
  let rejectTransfer = (_error: Error) => {};

  const promise = new Promise<string>((resolve, reject) => {
    const settleResolve = (value: string) => {
      if (!settled) {
        settled = true;
        resolve(value);
      }
    };
    const settleReject = (error: Error) => {
      if (!settled) {
        settled = true;
        reject(error);
      }
    };
    rejectTransfer = settleReject;

    upload = new Upload(new Blob([]), {
      endpoint,
      retryDelays: [...SHARED_ATTACHMENT_STORAGE_CONFIG.retryDelays],
      headers: { "x-signature": options.signedUploadToken },
      uploadSize: options.sizeBytes,
      uploadDataDuringCreation: true,
      removeFingerprintOnSuccess: true,
      fingerprint: async () => fingerprint,
      fileReader: {
        openFile: async () => ({
          size: options.sizeBytes,
          slice: async (start: number, end: number) => {
            if (
              !Number.isSafeInteger(start) ||
              !Number.isSafeInteger(end) ||
              start < 0 ||
              end <= start ||
              end > options.sizeBytes ||
              end - start > SHARED_ATTACHMENT_STORAGE_CONFIG.chunkSize
            ) {
              throw new Error("invalid shared attachment read range");
            }
            const bytes = await options.readRange(start, end);
            if (
              !(bytes instanceof Uint8Array) ||
              bytes.byteLength !== end - start
            ) {
              throw new Error("shared attachment read was incomplete");
            }
            return {
              value: new Blob([bytes]),
              done: end === options.sizeBytes,
            };
          },
          close: () => {},
        }),
      },
      metadata: {
        bucketName: SHARED_ATTACHMENT_STORAGE_CONFIG.bucketName,
        objectName: options.objectKey,
        contentType: options.contentType,
        cacheControl: "0",
        metadata: JSON.stringify({ plaintextSha256: options.sha256 }),
      },
      chunkSize: SHARED_ATTACHMENT_STORAGE_CONFIG.chunkSize,
      onError: settleReject,
      onProgress: (bytesUploaded, bytesTotal) => {
        if (options.onProgress && bytesTotal > 0) {
          options.onProgress((bytesUploaded / bytesTotal) * 100);
        }
      },
      onSuccess: () => settleResolve(options.objectKey),
    });

    upload
      .findPreviousUploads()
      .then((previousUploads) => {
        if (cancelled) return;
        const previous = previousUploads.find(
          (candidate) => candidate.size === options.sizeBytes,
        );
        if (previous) upload!.resumeFromPreviousUpload(previous);
        if (!cancelled) upload!.start();
      })
      .catch(settleReject);
  });

  return {
    promise,
    abort: async () => {
      if (cancelled || settled) return;
      cancelled = true;
      const error = new Error("shared attachment upload aborted");
      error.name = "AbortError";
      rejectTransfer(error);
      await upload?.abort();
    },
  };
}

function isSafeSharedContentType(value: string) {
  if (
    value.length === 0 ||
    value.length > 255 ||
    value !== value.toLowerCase().trim() ||
    [
      "text/html",
      "image/svg+xml",
      "application/xhtml+xml",
      "application/xml",
      "text/xml",
      "application/javascript",
      "text/javascript",
    ].includes(value)
  ) {
    return false;
  }
  const parts = value.split("/");
  return (
    parts.length === 2 &&
    parts.every((part) => part.length > 0 && /^[a-z0-9!#$&^_.+-]+$/.test(part))
  );
}
