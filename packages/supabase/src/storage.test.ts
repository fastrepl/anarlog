import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  abort: vi.fn().mockResolvedValue(undefined),
  findPreviousUploads: vi.fn().mockResolvedValue([]),
  instances: [] as Array<{
    file: unknown;
    options: Record<string, any>;
    resumeFromPreviousUpload: ReturnType<typeof vi.fn>;
    start: ReturnType<typeof vi.fn>;
  }>,
}));

vi.mock("tus-js-client", () => ({
  Upload: class {
    file: unknown;
    options: Record<string, any>;
    resumeFromPreviousUpload = vi.fn();
    start = vi.fn(() => this.options.onSuccess?.({}));

    constructor(file: unknown, options: Record<string, any>) {
      this.file = file;
      this.options = options;
      mocks.instances.push(this);
    }

    findPreviousUploads() {
      return mocks.findPreviousUploads();
    }

    abort() {
      return mocks.abort();
    }
  },
}));

import {
  PRIVATE_ATTACHMENT_STORAGE_CONFIG,
  uploadPrivateAttachment,
  uploadSharedAttachment,
} from "./storage";

const USER_ID = "11111111-1111-4111-8111-111111111111";
const OBJECT_ID = "22222222-2222-4222-8222-222222222222";
const OBJECT_KEY = `${USER_ID}/${OBJECT_ID}.anb1`;

function startUpload(
  overrides: Partial<Parameters<typeof uploadPrivateAttachment>[0]> = {},
) {
  return uploadPrivateAttachment({
    objectKey: OBJECT_KEY,
    signedUploadToken: "signed-token",
    ciphertextSha256: "a".repeat(64),
    ciphertextSizeBytes: 12,
    supabaseUrl: "https://project.supabase.co",
    readRange: vi.fn(async (start, end) => new Uint8Array(end - start).fill(7)),
    ...overrides,
  });
}

describe("private attachment storage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.instances.length = 0;
    mocks.findPreviousUploads.mockResolvedValue([]);
  });

  it("uploads immutable ciphertext with a signed TUS grant", async () => {
    const onProgress = vi.fn();
    const transfer = startUpload({ onProgress });
    const instance = mocks.instances[0]!;

    instance.options.onProgress(3, 12);
    await expect(transfer.promise).resolves.toBe(OBJECT_KEY);

    expect(instance.options.endpoint).toBe(
      "https://project.storage.supabase.co/storage/v1/upload/resumable/sign",
    );
    expect(instance.options.headers).toEqual({
      "x-signature": "signed-token",
    });
    expect(instance.options.headers).not.toHaveProperty("authorization");
    expect(instance.options.headers).not.toHaveProperty("x-upsert");
    expect(instance.options.uploadSize).toBe(12);
    expect(instance.options.chunkSize).toBe(6 * 1024 * 1024);
    expect(instance.options.metadata).toMatchObject({
      bucketName: "attachment-backups",
      objectName: OBJECT_KEY,
      contentType: "application/octet-stream",
      cacheControl: "0",
    });
    expect(JSON.parse(instance.options.metadata.metadata)).toEqual({
      formatVersion: 1,
      ciphertextSha256: "a".repeat(64),
    });
    expect(onProgress).toHaveBeenCalledWith(25);
  });

  it("reads only the bounded ciphertext range requested by TUS", async () => {
    const readRange = vi.fn(async () => new Uint8Array([1, 2, 3, 4]));
    startUpload({ ciphertextSizeBytes: 4, readRange });
    const source = await mocks.instances[0]!.options.fileReader.openFile();

    const result = await source.slice(0, 4);
    expect(result.done).toBe(true);
    expect(result.value).toBeInstanceOf(Blob);
    expect(result.value.size).toBe(4);
    expect(new Uint8Array(await result.value.arrayBuffer())).toEqual(
      new Uint8Array([1, 2, 3, 4]),
    );
    expect(readRange).toHaveBeenCalledWith(0, 4);
    await expect(source.slice(0, 5)).rejects.toThrow("read range");
    await expect(
      source.slice(0, PRIVATE_ATTACHMENT_STORAGE_CONFIG.chunkSize + 1),
    ).rejects.toThrow("read range");
  });

  it("fails closed when Rust returns an incomplete chunk", async () => {
    startUpload({
      ciphertextSizeBytes: 4,
      readRange: vi.fn(async () => new Uint8Array([1, 2, 3])),
    });
    const source = await mocks.instances[0]!.options.fileReader.openFile();

    await expect(source.slice(0, 4)).rejects.toThrow("incomplete");
  });

  it("resumes only the same ciphertext size", async () => {
    const matching = { size: 12, uploadUrl: "matching" };
    mocks.findPreviousUploads.mockResolvedValue([
      { size: 9, uploadUrl: "different" },
      matching,
    ]);

    const transfer = startUpload();
    await transfer.promise;

    expect(mocks.instances[0]!.resumeFromPreviousUpload).toHaveBeenCalledWith(
      matching,
    );
  });

  it("does not start after cancellation while resume lookup is pending", async () => {
    let resolvePreviousUploads!: (value: unknown[]) => void;
    mocks.findPreviousUploads.mockReturnValue(
      new Promise((resolve) => {
        resolvePreviousUploads = resolve;
      }),
    );
    const transfer = startUpload();
    const rejection = expect(transfer.promise).rejects.toMatchObject({
      name: "AbortError",
    });

    await transfer.abort();
    resolvePreviousUploads([]);
    await rejection;
    await Promise.resolve();

    expect(mocks.instances[0]!.start).not.toHaveBeenCalled();
    expect(mocks.abort).toHaveBeenCalledOnce();
  });

  it("settles the wrapper promise when an in-flight upload is aborted", async () => {
    const transfer = startUpload();
    const instance = mocks.instances[0]!;
    instance.start.mockImplementation(() => {});
    await Promise.resolve();
    expect(instance.start).toHaveBeenCalledOnce();

    const rejection = expect(transfer.promise).rejects.toMatchObject({
      name: "AbortError",
    });
    await transfer.abort();
    await rejection;
  });

  it("feeds sized Blob chunks to the pinned TUS client", async () => {
    startUpload({
      ciphertextSizeBytes: 4,
      readRange: vi.fn(async () => new Uint8Array([1, 2, 3, 4])),
    });
    const fileReader = mocks.instances[0]!.options.fileReader;
    const { Upload: ActualUpload } =
      await vi.importActual<typeof import("tus-js-client")>("tus-js-client");
    const requestBodies: Blob[] = [];
    const response = {
      getStatus: () => 201,
      getHeader: (name: string) => {
        if (name.toLowerCase() === "location") {
          return "/storage/v1/upload/resumable/upload-id";
        }
        if (name.toLowerCase() === "upload-offset") {
          return "4";
        }
        return undefined;
      },
      getBody: () => "",
      getUnderlyingObject: () => null,
    };
    const httpStack = {
      getName: () => "test-http-stack",
      createRequest: (method: string, url: string) => {
        const headers = new Map<string, string>();
        let reportProgress = (_bytesSent: number) => {};
        return {
          getMethod: () => method,
          getURL: () => url,
          setHeader: (name: string, value: string) => {
            headers.set(name, value);
          },
          getHeader: (name: string) => headers.get(name),
          setProgressHandler: (handler: (bytesSent: number) => void) => {
            reportProgress = handler;
          },
          send: async (body: Blob) => {
            requestBodies.push(body);
            reportProgress(body.size);
            return response;
          },
          abort: async () => {},
          getUnderlyingObject: () => null,
        };
      },
    };

    await new Promise<void>((resolve, reject) => {
      new ActualUpload(new Blob([]), {
        endpoint:
          "https://project.storage.supabase.co/storage/v1/upload/resumable/sign",
        fileReader,
        httpStack,
        uploadSize: 4,
        chunkSize: PRIVATE_ATTACHMENT_STORAGE_CONFIG.chunkSize,
        uploadDataDuringCreation: true,
        storeFingerprintForResuming: false,
        retryDelays: null,
        onSuccess: () => resolve(),
        onError: reject,
      }).start();
    });

    expect(requestBodies).toHaveLength(1);
    expect(requestBodies[0]).toBeInstanceOf(Blob);
    expect(requestBodies[0]!.size).toBe(4);
  });

  it("rejects malformed grants and object metadata before upload", () => {
    expect(() => startUpload({ objectKey: "../object.anb1" })).toThrow(
      "object key",
    );
    expect(() => startUpload({ signedUploadToken: "" })).toThrow(
      "upload token",
    );
    expect(() => startUpload({ ciphertextSha256: "not-a-checksum" })).toThrow(
      "checksum",
    );
    expect(() => startUpload({ ciphertextSizeBytes: 0 })).toThrow("size");
  });

  it("accepts UUID v7 attachment object keys", async () => {
    const objectKey = `${USER_ID}/22222222-2222-7222-8222-222222222222.anb1`;

    await expect(startUpload({ objectKey }).promise).resolves.toBe(objectKey);
  });
});

describe("shared attachment storage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.instances.length = 0;
    mocks.findPreviousUploads.mockResolvedValue([]);
  });

  it("uploads plaintext through the signed TUS endpoint", async () => {
    const objectKey = [
      "11111111-1111-4111-8111-111111111111",
      "22222222-2222-4222-8222-222222222222",
      "33333333-3333-4333-8333-333333333333.sna1",
    ].join("/");
    const transfer = uploadSharedAttachment({
      objectKey,
      signedUploadToken: "signed-token",
      contentType: "image/png",
      sha256: "b".repeat(64),
      sizeBytes: 12,
      supabaseUrl: "https://project.supabase.co",
      readRange: vi.fn(async (start, end) => new Uint8Array(end - start)),
    });
    const instance = mocks.instances[0]!;

    await expect(transfer.promise).resolves.toBe(objectKey);
    expect(instance.options.endpoint).toBe(
      "https://project.storage.supabase.co/storage/v1/upload/resumable/sign",
    );
    expect(instance.options.headers).toEqual({
      "x-signature": "signed-token",
    });
    expect(instance.options.metadata).toMatchObject({
      bucketName: "shared-note-attachments",
      objectName: objectKey,
      contentType: "image/png",
      cacheControl: "0",
    });
    expect(JSON.parse(instance.options.metadata.metadata)).toEqual({
      plaintextSha256: "b".repeat(64),
    });
  });
});
