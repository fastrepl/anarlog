import type { Session } from "@supabase/supabase-js";
import { describe, expect, it, vi } from "vitest";

import {
  createSharedAttachmentClient,
  SharedAttachmentGatewayError,
} from "./attachment-client";

const shareId = "11111111-1111-4111-8111-111111111111";
const attachmentId = "22222222-2222-4222-8222-222222222222";
const session = {
  access_token: "access-token",
  token_type: "bearer",
} as Session;

function response(overrides: Record<string, unknown> = {}) {
  return new Response(
    JSON.stringify({
      id: attachmentId,
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 42,
      sha256: "a".repeat(64),
      signedUrl:
        "https://project.supabase.co/storage/v1/object/sign/shared/file?token=one",
      expiresAt: "2026-07-17T12:01:00.000Z",
      ...overrides,
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

describe("shared attachment client", () => {
  it("requests a short-lived download grant with the current session", async () => {
    const fetcher = vi.fn().mockResolvedValue(response());
    const client = createSharedAttachmentClient({
      apiBaseUrl: "https://api.anarlog.so",
      session,
      fetcher,
    });

    await expect(client.download(shareId, attachmentId)).resolves.toMatchObject(
      {
        id: attachmentId,
        sha256: "a".repeat(64),
      },
    );
    expect(fetcher).toHaveBeenCalledWith(
      new URL(
        `https://api.anarlog.so/shared-notes/access/${shareId}/attachments/${attachmentId}/download`,
      ),
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "bearer access-token",
        }),
        credentials: "omit",
        redirect: "error",
      }),
    );
  });

  it("rejects non-HTTPS or credential-bearing signed URLs", async () => {
    for (const signedUrl of [
      "http://project.supabase.co/storage/file",
      "https://user:secret@project.supabase.co/storage/file",
      "https://project.supabase.co/storage/file#fragment",
    ]) {
      const client = createSharedAttachmentClient({
        apiBaseUrl: "https://api.anarlog.so",
        session,
        fetcher: vi.fn().mockResolvedValue(response({ signedUrl })),
      });
      await expect(client.download(shareId, attachmentId)).rejects.toThrow(
        "invalid shared attachment",
      );
    }
  });

  it("preserves gateway status for revocation handling", async () => {
    const client = createSharedAttachmentClient({
      apiBaseUrl: "https://api.anarlog.so",
      session,
      fetcher: vi.fn().mockResolvedValue(new Response(null, { status: 404 })),
    });

    await expect(client.download(shareId, attachmentId)).rejects.toEqual(
      new SharedAttachmentGatewayError(404),
    );
  });
});
