import {
  act,
  cleanup,
  render,
  renderHook,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: null as any,
  downloadSharedAttachment: vi.fn(),
  clearSharedAttachmentScope: vi.fn().mockResolvedValue(0),
  clearSharedAttachmentPreviewScopes: vi
    .fn()
    .mockResolvedValue({ status: "ok", data: false }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset:${path}`,
}));

vi.mock("~/attachment-sync/native", () => ({
  attachmentTransferNative: {
    downloadSharedAttachment: mocks.downloadSharedAttachment,
    clearSharedAttachmentScope: mocks.clearSharedAttachmentScope,
  },
}));

vi.mock("@hypr/plugin-attachment-sync", () => ({
  commands: {
    clearSharedAttachmentPreviewScopes:
      mocks.clearSharedAttachmentPreviewScopes,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session }),
}));

vi.mock("~/env", () => ({
  env: {
    VITE_API_URL: "https://api.test",
    VITE_SUPABASE_URL: "https://project.supabase.co",
  },
}));

import {
  beginSharedNotePreview,
  claimSharedNoteHandoff,
  parseSharedNotePreviewSnapshot,
  purgeAllSharedNotePreviews,
  purgeSharedNotePreview,
  SharedNotePreviewAuthLifecycle,
  useSharedNotePreview,
} from "./preview";

const viewId = "13697a87-f69b-456d-8679-4202d4f5d498";
const requestId = "44db4d75-2c8d-4b37-a60c-a3dc8b194c38";
const leaseId = "0f7890b4-42e6-4ccd-9374-ce1df2933157";
const serverSnapshot = {
  shareId: "f733dd21-336b-4b99-8967-c1e05509268e",
  schemaVersion: 1,
  contentRevision: 3,
  title: "Shared plan",
  body: { type: "doc", content: [{ type: "paragraph" }] },
  publishedAt: "2026-07-17T10:00:00.000Z",
};
type ClaimResult = Awaited<ReturnType<typeof claimSharedNoteHandoff>>;

describe("ephemeral shared-note previews", () => {
  beforeEach(() => {
    purgeAllSharedNotePreviews();
    vi.clearAllMocks();
    mocks.session = { user: { id: "viewer-1" } };
    mocks.downloadSharedAttachment.mockResolvedValue({
      cacheId: "cache-id",
      localPath: "/cache/attachment.bin",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    });
  });

  afterEach(() => {
    cleanup();
    purgeAllSharedNotePreviews();
  });

  it("accepts only the minimal link or public snapshot", () => {
    expect(parseSharedNotePreviewSnapshot(serverSnapshot)).toEqual({
      shareId: serverSnapshot.shareId,
      schemaVersion: 1,
      contentRevision: 3,
      title: "Shared plan",
      body: serverSnapshot.body,
      attachments: [],
      attachmentDownloads: [],
      publishedAt: "2026-07-17T10:00:00.000Z",
    });
    expect(() =>
      parseSharedNotePreviewSnapshot({
        ...serverSnapshot,
        session_id: "private-session",
      }),
    ).toThrow("invalid shared-note preview snapshot");
  });

  it("rejects server-supplied batch download grants", () => {
    const attachment = {
      id: "8df61ab1-3f8b-4218-a947-a5d2dbc579ef",
      filename: "recording.m4a",
      contentType: "audio/mp4",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    expect(() =>
      parseSharedNotePreviewSnapshot({
        ...serverSnapshot,
        attachments: [attachment],
        attachmentDownloads: [
          {
            ...attachment,
            signedUrl:
              "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=one",
            expiresAt: "2026-07-17T10:01:00.000Z",
          },
        ],
      }),
    ).toThrow("invalid shared-note preview snapshot");
  });

  it("claims a handoff once without including identifiers in stored content", async () => {
    const fetcher = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          ...serverSnapshot,
          leaseExpiresAt: "2099-07-17T10:20:00.000Z",
        }),
        {
          status: 200,
          headers: { "content-type": "application/json" },
        },
      ),
    );

    const claim = await claimSharedNoteHandoff(
      requestId,
      new AbortController().signal,
      fetcher,
      () => leaseId,
    );

    expect(fetcher).toHaveBeenCalledWith(
      new URL("https://api.test/shared-notes/handoffs/claim"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ requestId, leaseId }),
        cache: "no-store",
        credentials: "omit",
        redirect: "error",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
      }),
    );
    expect(String(fetcher.mock.calls[0]?.[0])).not.toContain(requestId);
    expect(JSON.stringify(claim.snapshot)).not.toContain(requestId);
    expect(JSON.stringify(claim.snapshot)).not.toContain(leaseId);
  });

  it("retries a lost claim response with the same generated lease", async () => {
    const fetcher = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("connection reset"))
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ...serverSnapshot,
            leaseExpiresAt: "2099-07-17T10:20:00.000Z",
          }),
          { status: 200 },
        ),
      );
    const createLeaseId = vi.fn(() => leaseId);

    await claimSharedNoteHandoff(
      requestId,
      new AbortController().signal,
      fetcher,
      createLeaseId,
    );

    expect(createLeaseId).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls.map((call) => call[1]?.body)).toEqual([
      JSON.stringify({ requestId, leaseId }),
      JSON.stringify({ requestId, leaseId }),
    ]);
  });

  it("requests and validates one leased attachment grant at download time", async () => {
    const attachment = {
      id: "8df61ab1-3f8b-4218-a947-a5d2dbc579ef",
      filename: "recording.m4a",
      contentType: "audio/mp4",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ...serverSnapshot,
            attachments: [attachment],
            leaseExpiresAt: "2099-07-17T10:20:00.000Z",
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ...attachment,
            signedUrl:
              "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=one",
            expiresAt: "2099-07-17T10:01:00.000Z",
          }),
          { status: 200 },
        ),
      );

    const claim = await claimSharedNoteHandoff(
      requestId,
      new AbortController().signal,
      fetcher,
      () => leaseId,
    );
    const download = await claim.downloadAttachment(
      attachment,
      new AbortController().signal,
    );

    expect(download.id).toBe(attachment.id);
    expect(fetcher.mock.calls[1]?.[0]).toEqual(
      new URL(
        `https://api.test/shared-notes/handoffs/attachments/${attachment.id}/download`,
      ),
    );
    expect(fetcher.mock.calls[1]?.[1]).toEqual(
      expect.objectContaining({
        body: JSON.stringify({ leaseId }),
        credentials: "omit",
      }),
    );
    expect(String(fetcher.mock.calls[1]?.[0])).not.toContain(leaseId);
  });

  it("rejects deeply nested preview documents", () => {
    let body: Record<string, unknown> = { type: "paragraph" };
    for (let index = 0; index < 65; index += 1) {
      body = { type: "blockquote", content: [body] };
    }

    expect(() =>
      parseSharedNotePreviewSnapshot({ ...serverSnapshot, body }),
    ).toThrow("invalid shared-note preview snapshot");
  });

  it("rejects preview documents with too many nodes", () => {
    const body = {
      type: "doc",
      content: Array.from({ length: 50_000 }, () => ({ type: "paragraph" })),
    };

    expect(() =>
      parseSharedNotePreviewSnapshot({ ...serverSnapshot, body }),
    ).toThrow("invalid shared-note preview snapshot");
  });

  it("keeps loading and ready data in process memory only", async () => {
    let resolveClaim:
      | ((value: {
          snapshot: ReturnType<typeof parseSharedNotePreviewSnapshot>;
          downloadAttachment: ClaimResult["downloadAttachment"];
        }) => void)
      | undefined;
    const claim = vi.fn(
      () =>
        new Promise<{
          snapshot: ReturnType<typeof parseSharedNotePreviewSnapshot>;
          downloadAttachment: ClaimResult["downloadAttachment"];
        }>((resolve) => {
          resolveClaim = resolve;
        }),
    );
    const { result } = renderHook(() => useSharedNotePreview(viewId));

    act(() => {
      beginSharedNotePreview(claim, () => viewId);
    });
    expect(result.current.status).toBe("loading");

    await act(async () => {
      resolveClaim?.({
        snapshot: parseSharedNotePreviewSnapshot(serverSnapshot),
        downloadAttachment: vi.fn() as ClaimResult["downloadAttachment"],
      });
    });
    expect(result.current.status).toBe("ready");

    act(() => purgeSharedNotePreview(viewId));
    expect(result.current.status).toBe("unavailable");
  });

  it("caches handoff attachments before their one-time grants expire", async () => {
    const attachment = {
      id: "8df61ab1-3f8b-4218-a947-a5d2dbc579ef",
      filename: "recording.m4a",
      contentType: "audio/mp4",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    const snapshot = parseSharedNotePreviewSnapshot({
      ...serverSnapshot,
      attachments: [attachment],
    });
    const downloadAttachment = vi.fn().mockResolvedValue({
      ...attachment,
      signedUrl:
        "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=one",
      expiresAt: "2099-07-17T10:05:00.000Z",
    });
    const preview = renderHook(() => useSharedNotePreview(viewId));

    act(() => {
      beginSharedNotePreview(
        async () => ({ snapshot, downloadAttachment }),
        () => viewId,
      );
    });

    await waitFor(() =>
      expect(
        preview.result.current.status === "ready"
          ? preview.result.current.snapshot.attachmentDownloads[0]?.localPath
          : null,
      ).toBe("/cache/attachment.bin"),
    );
    expect(downloadAttachment).toHaveBeenCalledWith(
      attachment,
      expect.any(AbortSignal),
    );
    expect(downloadAttachment.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.downloadSharedAttachment.mock.invocationCallOrder[0] ?? Infinity,
    );
    expect(mocks.downloadSharedAttachment).toHaveBeenCalledWith(
      {
        scopeId: `preview:${viewId}`,
        attachmentId: attachment.id,
        signedUrl: expect.stringContaining("project.supabase.co"),
        expectedSha256: attachment.sha256,
        expectedSizeBytes: attachment.sizeBytes,
      },
      expect.any(AbortSignal),
    );
  });

  it("clears files written after an in-flight preview is purged", async () => {
    const attachment = {
      id: "8df61ab1-3f8b-4218-a947-a5d2dbc579ef",
      filename: "recording.m4a",
      contentType: "audio/mp4",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    const snapshot = parseSharedNotePreviewSnapshot({
      ...serverSnapshot,
      attachments: [attachment],
    });
    let resolveDownload:
      | ((value: {
          cacheId: string;
          localPath: string;
          sizeBytes: number;
          sha256: string;
        }) => void)
      | undefined;
    mocks.downloadSharedAttachment.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveDownload = resolve;
        }),
    );

    beginSharedNotePreview(
      async () => ({
        snapshot,
        downloadAttachment: vi.fn().mockResolvedValue({
          ...attachment,
          signedUrl:
            "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=one",
          expiresAt: "2099-07-17T10:05:00.000Z",
        }),
      }),
      () => viewId,
    );
    await waitFor(() =>
      expect(mocks.downloadSharedAttachment).toHaveBeenCalledTimes(1),
    );

    purgeSharedNotePreview(viewId);
    expect(mocks.clearSharedAttachmentScope).toHaveBeenCalledTimes(1);

    resolveDownload?.({
      cacheId: "cache-id",
      localPath: "/cache/attachment.bin",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    });
    await waitFor(() =>
      expect(mocks.clearSharedAttachmentScope).toHaveBeenCalledTimes(2),
    );
    expect(mocks.clearSharedAttachmentScope).toHaveBeenNthCalledWith(
      1,
      `preview:${viewId}`,
    );
    expect(mocks.clearSharedAttachmentScope).toHaveBeenNthCalledWith(
      2,
      `preview:${viewId}`,
    );
  });

  it("retries transient grant and download failures with the same lease", async () => {
    const attachment = {
      id: "8df61ab1-3f8b-4218-a947-a5d2dbc579ef",
      filename: "recording.m4a",
      contentType: "audio/mp4",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    const download = {
      ...attachment,
      signedUrl:
        "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=one",
      expiresAt: "2099-07-17T10:05:00.000Z",
    };
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ...serverSnapshot,
            attachments: [attachment],
            leaseExpiresAt: "2099-07-17T10:20:00.000Z",
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(new Response(null, { status: 502 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify(download), { status: 200 }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(download), { status: 200 }),
      );
    mocks.downloadSharedAttachment
      .mockRejectedValueOnce(new Error("temporary download failure"))
      .mockResolvedValueOnce({
        cacheId: "cache-id",
        localPath: "/cache/attachment.bin",
        sizeBytes: 42,
        sha256: "a".repeat(64),
      });
    const claim = await claimSharedNoteHandoff(
      requestId,
      new AbortController().signal,
      fetcher,
      () => leaseId,
    );
    const preview = renderHook(() => useSharedNotePreview(viewId));

    act(() => {
      beginSharedNotePreview(
        async () => claim,
        () => viewId,
      );
    });

    await waitFor(() =>
      expect(
        preview.result.current.status === "ready"
          ? preview.result.current.snapshot.attachmentDownloads[0]?.localPath
          : null,
      ).toBe("/cache/attachment.bin"),
    );
    expect(mocks.downloadSharedAttachment).toHaveBeenCalledTimes(2);
    expect(mocks.downloadSharedAttachment).toHaveBeenCalledWith(
      {
        scopeId: `preview:${viewId}`,
        attachmentId: attachment.id,
        signedUrl: expect.stringContaining("project.supabase.co"),
        expectedSha256: attachment.sha256,
        expectedSizeBytes: attachment.sizeBytes,
      },
      expect.any(AbortSignal),
    );
    expect(fetcher).toHaveBeenCalledTimes(4);
    expect(fetcher.mock.calls.slice(1).map((call) => call[1]?.body)).toEqual([
      JSON.stringify({ leaseId }),
      JSON.stringify({ leaseId }),
      JSON.stringify({ leaseId }),
    ]);
  });

  it("aborts an in-flight claim when the preview is purged", () => {
    let signal: AbortSignal | undefined;
    const claim = vi.fn((nextSignal: AbortSignal) => {
      signal = nextSignal;
      return new Promise<never>(() => {});
    });

    beginSharedNotePreview(claim, () => viewId);
    purgeSharedNotePreview(viewId);

    expect(signal?.aborted).toBe(true);
  });

  it("clears the preview scope again after a deferred download settles", async () => {
    let resolveDownload:
      | ((value: {
          cacheId: string;
          localPath: string;
          sizeBytes: number;
          sha256: string;
        }) => void)
      | undefined;
    mocks.downloadSharedAttachment.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveDownload = resolve;
      }),
    );
    const attachment = {
      id: "8df61ab1-3f8b-4218-a947-a5d2dbc579ef",
      filename: "recording.m4a",
      contentType: "audio/mp4",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    const snapshot = parseSharedNotePreviewSnapshot({
      ...serverSnapshot,
      attachments: [attachment],
    });
    const downloadAttachment = vi.fn().mockResolvedValue({
      ...attachment,
      signedUrl:
        "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=one",
      expiresAt: "2026-07-17T10:05:00.000Z",
    });

    beginSharedNotePreview(
      async () => ({ snapshot, downloadAttachment }),
      () => viewId,
    );
    await waitFor(() =>
      expect(mocks.downloadSharedAttachment).toHaveBeenCalledOnce(),
    );

    purgeSharedNotePreview(viewId);
    expect(mocks.clearSharedAttachmentScope).toHaveBeenCalledTimes(1);

    resolveDownload?.({
      cacheId: "cache-id",
      localPath: "/cache/attachment.bin",
      sizeBytes: 42,
      sha256: attachment.sha256,
    });
    await waitFor(() =>
      expect(mocks.clearSharedAttachmentScope).toHaveBeenCalledTimes(2),
    );
    expect(mocks.clearSharedAttachmentScope).toHaveBeenNthCalledWith(
      2,
      `preview:${viewId}`,
    );
  });

  it("clears crash-orphaned preview scopes on startup", async () => {
    render(<SharedNotePreviewAuthLifecycle />);

    await waitFor(() =>
      expect(mocks.clearSharedAttachmentPreviewScopes).toHaveBeenCalledOnce(),
    );
  });

  it("purges previews when the signed-in account changes", () => {
    const claim = vi.fn(() => new Promise<never>(() => {}));
    const preview = renderHook(() => useSharedNotePreview(viewId));
    beginSharedNotePreview(claim, () => viewId);
    const lifecycle = render(<SharedNotePreviewAuthLifecycle />);
    expect(preview.result.current.status).toBe("loading");

    mocks.session = null;
    lifecycle.rerender(<SharedNotePreviewAuthLifecycle />);

    expect(preview.result.current.status).toBe("unavailable");
  });
});
