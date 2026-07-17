import { Upload } from "tus-js-client";

export const STORAGE_CONFIG = {
  bucketName: "audio-files",
  chunkSize: 6 * 1024 * 1024,
  retryDelays: [0, 3000, 5000, 10000, 20000],
} as const;

export function getTusEndpoint(supabaseUrl: string): string {
  const parsed = new URL(supabaseUrl);
  const isHostedSupabase = parsed.hostname.endsWith(".supabase.co");

  if (isHostedSupabase) {
    const projectId = parsed.hostname.split(".")[0];
    return `https://${projectId}.storage.supabase.co/storage/v1/upload/resumable`;
  }

  return new URL("/storage/v1/upload/resumable", parsed.origin).toString();
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
