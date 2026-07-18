import { describe, expect, it, vi } from "vitest";

import {
  AttachmentBackupGatewayError,
  createAttachmentBackupClient,
  isAttachmentBackupDependencyAppeared,
  isAttachmentBackupDeleteCancelled,
} from "./client";

const deleteRequest = {
  objectKey: "owner/object.anb1",
  attachmentRef: "attachment-ref",
  versionRef: "version-ref",
  deleteRequestId: "11111111-1111-4111-8111-111111111111",
};

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

  it("sends the exact stable delete tuple for schedule and cancellation", async () => {
    const fetcher = vi.fn(
      async (url: URL | RequestInfo, init?: RequestInit) => {
        expect(JSON.parse(String(init?.body))).toEqual(deleteRequest);
        if (url.toString().endsWith("/delete/cancel")) {
          return Response.json(deleteRequest);
        }
        return Response.json({
          ...deleteRequest,
          deleteFenceId: "22222222-2222-4222-8222-222222222222",
          deleteGeneration: 3,
          deleteNotBefore: "2026-07-19T12:00:00.000Z",
        });
      },
    );
    const gateway = client(fetcher as typeof fetch);

    await expect(gateway.scheduleDelete(deleteRequest)).resolves.toMatchObject({
      deleteRequestId: deleteRequest.deleteRequestId,
      deleteGeneration: 3,
    });
    await expect(gateway.cancelDelete(deleteRequest)).resolves.toMatchObject({
      deleteRequestId: deleteRequest.deleteRequestId,
    });

    expect(fetcher.mock.calls[0]![0].toString()).toBe(
      "https://api.example.com/sync/attachment-backups/delete",
    );
    expect(fetcher.mock.calls[1]![0].toString()).toBe(
      "https://api.example.com/sync/attachment-backups/delete/cancel",
    );
  });

  it("treats absent heads and schedules as idempotent but fails closed on cancel", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(
          JSON.stringify({ error: { code: "attachment_backup_not_found" } }),
          { status: 404 },
        ),
    );
    const gateway = client(fetcher as typeof fetch);

    await expect(gateway.head("attachment-ref")).resolves.toBeNull();
    await expect(gateway.scheduleDelete(deleteRequest)).resolves.toBeNull();
    await expect(gateway.cancelDelete(deleteRequest)).rejects.toEqual(
      expect.objectContaining<Partial<AttachmentBackupGatewayError>>({
        status: 503,
        code: "attachment_backup_cancel_unavailable",
      }),
    );
  });

  it("reads typed nested API error codes with a top-level fallback", async () => {
    const nested = client(
      vi.fn(async () =>
        Response.json(
          {
            error: {
              code: "attachment_backup_dependency_appeared",
              message: "dependency appeared",
            },
          },
          { status: 409 },
        ),
      ) as typeof fetch,
    );
    const nestedError = await nested
      .scheduleDelete(deleteRequest)
      .catch((error: unknown) => error);
    expect(isAttachmentBackupDependencyAppeared(nestedError)).toBe(true);

    const cancelled = client(
      vi.fn(async () =>
        Response.json(
          { error: { code: "attachment_backup_delete_cancelled" } },
          { status: 409 },
        ),
      ) as typeof fetch,
    );
    const cancelledError = await cancelled
      .scheduleDelete(deleteRequest)
      .catch((error: unknown) => error);
    expect(isAttachmentBackupDeleteCancelled(cancelledError)).toBe(true);
    expect(isAttachmentBackupDependencyAppeared(cancelledError)).toBe(false);

    const topLevel = client(
      vi.fn(async () =>
        Response.json({ code: "attachment_backup_conflict" }, { status: 409 }),
      ) as typeof fetch,
    );
    await expect(topLevel.scheduleDelete(deleteRequest)).rejects.toEqual(
      expect.objectContaining<Partial<AttachmentBackupGatewayError>>({
        code: "attachment_backup_conflict",
      }),
    );
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
