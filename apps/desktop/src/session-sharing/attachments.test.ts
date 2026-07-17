import { describe, expect, it, vi } from "vitest";

import {
  addSharedAttachmentIds,
  isAttachmentShareable,
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

  it("adds only explicitly selected shared IDs to attachment nodes", () => {
    const body = addSharedAttachmentIds(
      {
        type: "doc",
        content: [
          {
            type: "image",
            attrs: {
              attachmentId: "local-attachment",
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
      new Map([["local-attachment", "33333333-3333-4333-8333-333333333333"]]),
    );

    expect(body.content?.[0]?.attrs?.sharedAttachmentId).toBe(
      "33333333-3333-4333-8333-333333333333",
    );
    expect(body.content?.[0]?.attrs).toEqual({
      sharedAttachmentId: "33333333-3333-4333-8333-333333333333",
    });
    expect(body.content?.[1]?.attrs).toEqual({});
  });

  it("restores local attachment IDs from a shared snapshot and fails closed when unmatched", () => {
    const sharedId = "33333333-3333-4333-8333-333333333333";
    const restored = restoreLocalAttachmentIds(
      {
        type: "doc",
        content: [
          {
            type: "image",
            attrs: { sharedAttachmentId: sharedId },
          },
        ],
      },
      new Map([["local-attachment", sharedId]]),
    );

    expect(restored.content?.[0]?.attrs).toEqual({
      attachmentId: "local-attachment",
    });
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
        new Map(),
      ),
    ).toThrow("unavailable locally");
  });
});
