import { act, cleanup, render, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: null as any,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session }),
}));

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.test" },
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
const serverSnapshot = {
  shareId: "f733dd21-336b-4b99-8967-c1e05509268e",
  schemaVersion: 1,
  contentRevision: 3,
  title: "Shared plan",
  body: { type: "doc", content: [{ type: "paragraph" }] },
  publishedAt: "2026-07-17T10:00:00.000Z",
};

describe("ephemeral shared-note previews", () => {
  beforeEach(() => {
    purgeAllSharedNotePreviews();
    mocks.session = { user: { id: "viewer-1" } };
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
      publishedAt: "2026-07-17T10:00:00.000Z",
    });
    expect(() =>
      parseSharedNotePreviewSnapshot({
        ...serverSnapshot,
        session_id: "private-session",
      }),
    ).toThrow("invalid shared-note preview snapshot");
  });

  it("claims a handoff once without including identifiers in stored content", async () => {
    const fetcher = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(serverSnapshot), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    const snapshot = await claimSharedNoteHandoff(
      requestId,
      new AbortController().signal,
      fetcher,
    );

    expect(fetcher).toHaveBeenCalledWith(
      new URL("https://api.test/shared-notes/handoffs/claim"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ requestId }),
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
    expect(JSON.stringify(snapshot)).not.toContain(requestId);
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
      | ((value: ReturnType<typeof parseSharedNotePreviewSnapshot>) => void)
      | undefined;
    const claim = vi.fn(
      () =>
        new Promise<ReturnType<typeof parseSharedNotePreviewSnapshot>>(
          (resolve) => {
            resolveClaim = resolve;
          },
        ),
    );
    const { result } = renderHook(() => useSharedNotePreview(viewId));

    act(() => {
      beginSharedNotePreview(claim, () => viewId);
    });
    expect(result.current.status).toBe("loading");

    await act(async () => {
      resolveClaim?.(parseSharedNotePreviewSnapshot(serverSnapshot));
    });
    expect(result.current.status).toBe("ready");

    act(() => purgeSharedNotePreview(viewId));
    expect(result.current.status).toBe("unavailable");
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
