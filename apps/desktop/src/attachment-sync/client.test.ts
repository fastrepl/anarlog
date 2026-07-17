import { describe, expect, it, vi } from "vitest";

import {
  AttachmentBackupGatewayError,
  createAttachmentBackupClient,
} from "./client";

function client(fetcher: typeof fetch) {
  return createAttachmentBackupClient({
    apiBaseUrl: "https://api.example.com",
    session: {
      access_token: "access-token",
      user: { id: "11111111-1111-4111-8111-111111111111" },
    } as any,
    fetcher,
  });
}

describe("attachment backup client", () => {
  it("sends authenticated reservation metadata to the sync gateway", async () => {
    const fetcher = vi.fn(
      async (url: URL | RequestInfo, init?: RequestInit) => {
        expect(url.toString()).toBe(
          "https://api.example.com/sync/attachment-backups/reserve",
        );
        expect(new Headers(init?.headers).get("Authorization")).toBe(
          "Bearer access-token",
        );
        expect(JSON.parse(String(init?.body))).toEqual({
          attachmentRef: "attachment-ref",
          versionRef: "version-ref",
          ciphertextSizeBytes: 58,
          formatVersion: 1,
        });
        return new Response(
          JSON.stringify({
            objectId: "object-1",
            objectKey: "owner/object.anb1",
            objectState: "reserved",
            ciphertextSizeBytes: 58,
            formatVersion: 1,
            ciphertextSha256: null,
          }),
        );
      },
    );

    await expect(
      client(fetcher as typeof fetch).reserve({
        attachmentRef: "attachment-ref",
        versionRef: "version-ref",
        ciphertextSizeBytes: 58,
        formatVersion: 1,
      }),
    ).resolves.toMatchObject({ objectId: "object-1" });
  });

  it("treats absent heads and deletes as idempotent", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(JSON.stringify({ code: "not_found" }), { status: 404 }),
    );
    const gateway = client(fetcher as typeof fetch);

    await expect(gateway.head("attachment-ref")).resolves.toBeNull();
    await expect(gateway.delete("owner/object.anb1")).resolves.toEqual({
      objectKey: "owner/object.anb1",
      wasMarked: false,
    });
  });

  it("rejects oversized gateway responses before parsing", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response("{}", {
          headers: { "content-length": String(64 * 1024 + 1) },
        }),
    );

    await expect(
      client(fetcher as typeof fetch).head("attachment-ref"),
    ).rejects.toEqual(
      expect.objectContaining<Partial<AttachmentBackupGatewayError>>({
        code: "response_too_large",
      }),
    );
  });
});
