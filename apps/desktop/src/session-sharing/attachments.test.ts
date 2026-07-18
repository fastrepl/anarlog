import { describe, expect, it, vi } from "vitest";

import {
  addSharedAttachmentIds,
  isAttachmentShareable,
  matchSharedAttachmentsToLocal,
  prepareSessionShareAttachment,
  restoreLocalAttachmentIds,
  type SessionShareAttachment,
} from "./attachments";

const attachment: SessionShareAttachment = {
  id: "local-attachment",
  filename: "diagram.png",
  contentType: "image/png",
  sizeBytes: 42,
  sha256: "a".repeat(64),
  sourceType: "note_upload",
  sourceId: "diagram.png",
  cloudSyncEnabled: true,
  cloudObjectKey:
    "11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222.anb1",
  localAvailability: "present",
  transferDirection: null,
  transferPhase: "completed",
  transferError: "",
};

describe("shared attachment selection", () => {
  it("requires a completed private backup before sharing", async () => {
    expect(isAttachmentShareable(attachment)).toBe(true);
    expect(
      isAttachmentShareable({
        ...attachment,
        cloudSyncEnabled: false,
        cloudObjectKey: "",
      }),
    ).toBe(false);
    expect(isAttachmentShareable({ ...attachment, cloudObjectKey: "" })).toBe(
      false,
    );

    const fetcher = vi.fn();
    await expect(
      prepareSessionShareAttachment({
        apiBaseUrl: "https://api.example.com",
        supabaseUrl: "https://project.supabase.co",
        session: {
          access_token: "token",
          user: { id: "11111111-1111-4111-8111-111111111111" },
        } as any,
        shareId: "22222222-2222-4222-8222-222222222222",
        attachment: { ...attachment, cloudObjectKey: "" },
        fetcher,
      }),
    ).rejects.toThrow("not available");
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("uploads every range from one verified native snapshot", async () => {
    const sharedAttachmentId = "33333333-3333-4333-8333-333333333333";
    const objectKey = `11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222/${sharedAttachmentId}.sna1`;
    const fetcher = vi.fn(async (url: URL | RequestInfo) => {
      const path = new URL(url.toString()).pathname;
      if (path.endsWith("/reserve")) {
        return Response.json({
          attachmentId: sharedAttachmentId,
          objectKey,
          objectState: "reserved",
          filename: attachment.filename,
          contentType: attachment.contentType,
          sizeBytes: attachment.sizeBytes,
          sha256: null,
        });
      }
      if (path.endsWith("/upload-grant")) {
        return Response.json({
          attachmentId: sharedAttachmentId,
          objectKey,
          objectState: "reserved",
          filename: attachment.filename,
          contentType: attachment.contentType,
          sizeBytes: attachment.sizeBytes,
          sha256: attachment.sha256,
          uploadToken: "signed-token",
        });
      }
      return Response.json({
        attachmentId: sharedAttachmentId,
        objectKey,
        objectState: "ready",
        wasFinalized: true,
      });
    });
    const native = {
      prepareSharedUpload: vi.fn().mockResolvedValue({
        cacheId: "44444444-4444-4444-8444-444444444444",
        sha256: attachment.sha256,
        sizeBytes: attachment.sizeBytes,
      }),
      readSharedUploadRange: vi
        .fn()
        .mockResolvedValue(new Uint8Array(attachment.sizeBytes)),
      validateSharedUpload: vi.fn().mockResolvedValue(true),
      cleanupSharedUpload: vi.fn().mockResolvedValue(true),
    };
    const abort = vi.fn(async () => {});
    const uploader = vi.fn((options) => ({
      promise: options
        .readRange(0, options.sizeBytes)
        .then(() => options.objectKey),
      abort,
    }));

    await expect(
      prepareSessionShareAttachment({
        apiBaseUrl: "https://api.example.com",
        supabaseUrl: "https://project.supabase.co",
        session: {
          access_token: "token",
          user: { id: "11111111-1111-4111-8111-111111111111" },
        } as any,
        shareId: "22222222-2222-4222-8222-222222222222",
        attachment,
        fetcher: fetcher as typeof fetch,
        uploader,
        native,
      }),
    ).resolves.toMatchObject({ id: sharedAttachmentId });

    expect(native.prepareSharedUpload).toHaveBeenCalledWith(
      attachment.id,
      attachment.sha256,
      attachment.sizeBytes,
      attachment.filename,
      attachment.contentType,
      attachment.cloudObjectKey,
      undefined,
    );
    expect(native.readSharedUploadRange).toHaveBeenCalledWith(
      attachment.id,
      "44444444-4444-4444-8444-444444444444",
      attachment.sha256,
      attachment.sizeBytes,
      attachment.filename,
      attachment.contentType,
      attachment.cloudObjectKey,
      0,
      attachment.sizeBytes,
    );
    expect(native.validateSharedUpload).toHaveBeenCalledTimes(2);
    expect(native.cleanupSharedUpload).toHaveBeenCalledWith(
      "44444444-4444-4444-8444-444444444444",
    );
    expect(
      fetcher.mock.calls.map(([url]) => new URL(url.toString()).pathname),
    ).toEqual([
      "/sync/shares/22222222-2222-4222-8222-222222222222/attachments/reserve",
      "/sync/shares/22222222-2222-4222-8222-222222222222/attachments/upload-grant",
      "/sync/shares/22222222-2222-4222-8222-222222222222/attachments/finalize",
    ]);
  });

  it("refuses to finalize a shared upload after the local version changes", async () => {
    const sharedAttachmentId = "33333333-3333-4333-8333-333333333333";
    const objectKey = `11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222/${sharedAttachmentId}.sna1`;
    const fetcher = vi.fn(async (url: URL | RequestInfo) => {
      const path = new URL(url.toString()).pathname;
      if (path.endsWith("/reserve")) {
        return Response.json({
          attachmentId: sharedAttachmentId,
          objectKey,
          objectState: "reserved",
          filename: attachment.filename,
          contentType: attachment.contentType,
          sizeBytes: attachment.sizeBytes,
          sha256: null,
        });
      }
      return Response.json({
        attachmentId: sharedAttachmentId,
        objectKey,
        objectState: "reserved",
        filename: attachment.filename,
        contentType: attachment.contentType,
        sizeBytes: attachment.sizeBytes,
        sha256: attachment.sha256,
        uploadToken: null,
      });
    });
    const native = {
      prepareSharedUpload: vi.fn().mockResolvedValue({
        cacheId: "44444444-4444-4444-8444-444444444444",
        sha256: attachment.sha256,
        sizeBytes: attachment.sizeBytes,
      }),
      readSharedUploadRange: vi.fn(),
      validateSharedUpload: vi.fn().mockResolvedValue(false),
      cleanupSharedUpload: vi.fn().mockResolvedValue(true),
    };

    await expect(
      prepareSessionShareAttachment({
        apiBaseUrl: "https://api.example.com",
        supabaseUrl: "https://project.supabase.co",
        session: {
          access_token: "token",
          user: { id: "11111111-1111-4111-8111-111111111111" },
        } as any,
        shareId: "22222222-2222-4222-8222-222222222222",
        attachment,
        fetcher: fetcher as typeof fetch,
        native,
      }),
    ).rejects.toThrow("changed during upload");
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(native.cleanupSharedUpload).toHaveBeenCalledOnce();
  });

  it("does not request an upload grant when cancellation wins during snapshotting", async () => {
    const controller = new AbortController();
    const sharedAttachmentId = "33333333-3333-4333-8333-333333333333";
    const objectKey = `11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222/${sharedAttachmentId}.sna1`;
    const fetcher = vi.fn(async () =>
      Response.json({
        attachmentId: sharedAttachmentId,
        objectKey,
        objectState: "reserved",
        filename: attachment.filename,
        contentType: attachment.contentType,
        sizeBytes: attachment.sizeBytes,
        sha256: null,
      }),
    );
    const native = {
      prepareSharedUpload: vi.fn(async () => {
        controller.abort();
        return {
          cacheId: "44444444-4444-4444-8444-444444444444",
          sha256: attachment.sha256,
          sizeBytes: attachment.sizeBytes,
        };
      }),
      readSharedUploadRange: vi.fn(),
      validateSharedUpload: vi.fn(),
      cleanupSharedUpload: vi.fn().mockResolvedValue(true),
    };

    await expect(
      prepareSessionShareAttachment({
        apiBaseUrl: "https://api.example.com",
        supabaseUrl: "https://project.supabase.co",
        session: {
          access_token: "token",
          user: { id: "11111111-1111-4111-8111-111111111111" },
        } as any,
        shareId: "22222222-2222-4222-8222-222222222222",
        attachment,
        fetcher: fetcher as typeof fetch,
        native,
        signal: controller.signal,
      }),
    ).rejects.toMatchObject({ name: "AbortError" });
    expect(fetcher).toHaveBeenCalledOnce();
    expect(native.cleanupSharedUpload).toHaveBeenCalledOnce();
  });

  it("drains an in-flight native range read before deleting its snapshot", async () => {
    const controller = new AbortController();
    const sharedAttachmentId = "33333333-3333-4333-8333-333333333333";
    const objectKey = `11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222/${sharedAttachmentId}.sna1`;
    const fetcher = vi.fn(async (url: URL | RequestInfo) => {
      const path = new URL(url.toString()).pathname;
      if (path.endsWith("/reserve")) {
        return Response.json({
          attachmentId: sharedAttachmentId,
          objectKey,
          objectState: "reserved",
          filename: attachment.filename,
          contentType: attachment.contentType,
          sizeBytes: attachment.sizeBytes,
          sha256: null,
        });
      }
      return Response.json({
        attachmentId: sharedAttachmentId,
        objectKey,
        objectState: "reserved",
        filename: attachment.filename,
        contentType: attachment.contentType,
        sizeBytes: attachment.sizeBytes,
        sha256: attachment.sha256,
        uploadToken: "signed-token",
      });
    });
    let finishRead!: (bytes: Uint8Array) => void;
    const native = {
      prepareSharedUpload: vi.fn().mockResolvedValue({
        cacheId: "44444444-4444-4444-8444-444444444444",
        sha256: attachment.sha256,
        sizeBytes: attachment.sizeBytes,
      }),
      readSharedUploadRange: vi.fn(
        () =>
          new Promise<Uint8Array>((resolve) => {
            finishRead = resolve;
          }),
      ),
      validateSharedUpload: vi.fn(),
      cleanupSharedUpload: vi.fn().mockResolvedValue(true),
    };
    let rejectUpload!: (reason: unknown) => void;
    const uploadPromise = new Promise<string>((_resolve, reject) => {
      rejectUpload = reject;
    });
    const abort = vi.fn(async () => {
      rejectUpload(controller.signal.reason);
    });
    const uploader = vi.fn((options) => {
      void options.readRange(0, options.sizeBytes);
      return { promise: uploadPromise, abort };
    });

    const operation = prepareSessionShareAttachment({
      apiBaseUrl: "https://api.example.com",
      supabaseUrl: "https://project.supabase.co",
      session: {
        access_token: "token",
        user: { id: "11111111-1111-4111-8111-111111111111" },
      } as any,
      shareId: "22222222-2222-4222-8222-222222222222",
      attachment,
      fetcher: fetcher as typeof fetch,
      uploader,
      native,
      signal: controller.signal,
    });
    await vi.waitFor(() => expect(uploader).toHaveBeenCalledOnce());

    controller.abort();
    await vi.waitFor(() => expect(abort).toHaveBeenCalledOnce());
    expect(native.cleanupSharedUpload).not.toHaveBeenCalled();
    finishRead(new Uint8Array(attachment.sizeBytes));

    await expect(operation).rejects.toMatchObject({ name: "AbortError" });
    expect(native.cleanupSharedUpload).toHaveBeenCalledOnce();
  });

  it("maps editor source identities while retaining catalog IDs for API operations", () => {
    const sharedId = "33333333-3333-4333-8333-333333333333";
    const localToShared = matchSharedAttachmentsToLocal(
      [attachment],
      [
        {
          id: sharedId,
          filename: attachment.filename,
          contentType: attachment.contentType,
          sizeBytes: attachment.sizeBytes,
          sha256: attachment.sha256,
        },
      ],
    );
    const body = addSharedAttachmentIds(
      {
        type: "doc",
        content: [
          {
            type: "image",
            attrs: {
              attachmentId: "diagram.png",
              src: "asset://local",
              path: "/Users/private/diagram.png",
            },
          },
          {
            type: "fileAttachment",
            attrs: { attachmentId: "private-attachment" },
          },
        ],
      },
      [attachment],
      localToShared,
    );

    expect(localToShared.get(attachment.id)).toBe(sharedId);
    expect(localToShared.get(attachment.sourceId)).toBeUndefined();
    expect(body.content?.[0]?.attrs?.sharedAttachmentId).toBe(sharedId);
    expect(body.content?.[0]?.attrs).toEqual({
      sharedAttachmentId: sharedId,
    });
    expect(body.content?.[1]?.attrs).toEqual({});
  });
  it("preserves YouTube clip sources instead of treating them as local files", () => {
    const clip = {
      type: "clip",
      attrs: {
        attachmentId: attachment.sourceId,
        src: "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
      },
    };

    const body = addSharedAttachmentIds(
      { type: "doc", content: [clip] },
      [attachment],
      new Map([[attachment.id, "33333333-3333-4333-8333-333333333333"]]),
    );

    expect(body.content?.[0]).toEqual(clip);
  });

  it("maps legacy files without letting session audio overwrite the source identity", () => {
    const audio = {
      ...attachment,
      id: "session-audio:session-1",
      filename: "meeting.m4a",
      contentType: "audio/mp4",
      sourceType: "session_audio",
      sourceId: "primary",
    };
    const legacy = {
      ...attachment,
      id: "legacy-attachment-id",
      sourceType: "legacy_file",
      sourceId: "primary",
    };
    const legacySharedId = "44444444-4444-4444-8444-444444444444";
    const body = addSharedAttachmentIds(
      {
        type: "doc",
        content: [
          {
            type: "image",
            attrs: { attachmentId: "primary", src: "asset://local" },
          },
        ],
      },
      [legacy, audio],
      new Map([
        [legacy.id, legacySharedId],
        [audio.id, "33333333-3333-4333-8333-333333333333"],
      ]),
    );

    expect(body.content?.[0]?.attrs).toEqual({
      sharedAttachmentId: legacySharedId,
    });
  });
  it("restores local attachment IDs from a shared snapshot and fails closed when unmatched", () => {
    const sharedId = "33333333-3333-4333-8333-333333333333";
    const fileSharedId = "44444444-4444-4444-8444-444444444444";
    const fileAttachment: SessionShareAttachment = {
      ...attachment,
      id: "local-file-record",
      filename: "notes.pdf",
      contentType: "application/pdf",
      sizeBytes: 84,
      sha256: "b".repeat(64),
      sourceId: "notes.pdf",
    };
    const localDocument = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            attachmentId: attachment.sourceId,
            src: "asset://local/diagram.png",
            alt: "Architecture diagram",
            title: "System overview",
            editorWidth: 42,
          },
        },
        {
          type: "fileAttachment",
          attrs: {
            attachmentId: fileAttachment.sourceId,
            src: "asset://local/notes.pdf",
            path: "/Users/private/notes.pdf",
            name: fileAttachment.filename,
            mimeType: fileAttachment.contentType,
            size: fileAttachment.sizeBytes,
          },
        },
        {
          type: "image",
          attrs: {
            attachmentId: attachment.sourceId,
            src: "asset://local/diagram.png",
            alt: "Architecture detail",
            title: "Second placement",
            editorWidth: 80,
          },
        },
      ],
    };
    const restored = restoreLocalAttachmentIds(
      {
        type: "doc",
        content: [
          {
            type: "image",
            attrs: { sharedAttachmentId: sharedId },
          },
          {
            type: "fileAttachment",
            attrs: { sharedAttachmentId: fileSharedId },
          },
          {
            type: "image",
            attrs: { sharedAttachmentId: sharedId },
          },
        ],
      },
      localDocument,
      [attachment, fileAttachment],
      new Map([
        [attachment.id, sharedId],
        [fileAttachment.id, fileSharedId],
      ]),
    );

    expect(restored).toEqual(localDocument);
    expect(() =>
      restoreLocalAttachmentIds(
        {
          type: "doc",
          content: [
            {
              type: "fileAttachment",
              attrs: { sharedAttachmentId: sharedId },
            },
          ],
        },
        localDocument,
        [attachment],
        new Map(),
      ),
    ).toThrow("unavailable locally");
  });
});
