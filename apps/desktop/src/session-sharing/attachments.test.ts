import { describe, expect, it, vi } from "vitest";

import {
  addSharedAttachmentIds,
  isAttachmentShareable,
  matchSharedAttachmentsToLocal,
  prepareSessionShareAttachment,
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
});
