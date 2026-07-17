import { describe, expect, it, vi } from "vitest";

import {
  AttachmentBackupGatewayError,
  createAttachmentBackupClient,
} from "./client";

function client(fetcher: typeof fetch) {
  return createAttachmentBackupClient({
    apiBaseUrl: "https://api.example.com",
    getAccessToken: () => "access-token",
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

  it("reads the current access token for every request", async () => {
    let accessToken = "first-token";
    const fetcher = vi.fn(
      async (_url: URL | RequestInfo, _init?: RequestInit) =>
        Response.json({
          versionRef: "version-ref",
          objectKey: "owner/object.anb1",
          ciphertextSha256: "a".repeat(64),
          ciphertextSizeBytes: 58,
          formatVersion: 1,
        }),
    );
    const gateway = createAttachmentBackupClient({
      apiBaseUrl: "https://api.example.com",
      getAccessToken: () => accessToken,
      fetcher: fetcher as typeof fetch,
    });

    await gateway.head("attachment-ref");
    accessToken = "refreshed-token";
    await gateway.head("attachment-ref");

    expect(
      new Headers(fetcher.mock.calls[0]![1]?.headers).get("Authorization"),
    ).toBe("Bearer first-token");
    expect(
      new Headers(fetcher.mock.calls[1]![1]?.headers).get("Authorization"),
    ).toBe("Bearer refreshed-token");
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
