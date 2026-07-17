import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  parseSnapshots: vi.fn(),
  upsertCache: vi.fn(),
  beginPreview: vi.fn(),
  claimHandoff: vi.fn(),
  purgePreview: vi.fn(),
}));

vi.mock("./cache", () => ({
  parseDurableSharedNoteSnapshots: mocks.parseSnapshots,
  upsertDurableSharedNoteCache: mocks.upsertCache,
}));

vi.mock("./preview", () => ({
  beginSharedNotePreview: mocks.beginPreview,
  claimSharedNoteHandoff: mocks.claimHandoff,
  purgeSharedNotePreview: mocks.purgePreview,
}));

import {
  createShareOpenProcessor,
  openAccountSharedNote,
  openHandoffSharedNote,
  subscribeThenDrainShareOpens,
} from "./deeplink";

const shareId = "82a163dd-d595-45f8-8d71-cf38bbb1ce12";
const requestId = "1b02e758-295d-4ea4-bd0f-6d3f68bcebf6";
const pendingId = "fe9f0e44-b283-48c1-9f53-a646f0bfd651";
const viewId = "b61233de-0ab6-4c30-9750-9516742fef60";

describe("shared-note desktop deep links", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.upsertCache.mockResolvedValue(undefined);
  });

  it("subscribes before draining cold-start requests", async () => {
    const order: string[] = [];
    const handle = vi.fn(async () => {
      order.push("handle");
    });

    const unlisten = await subscribeThenDrainShareOpens({
      listen: async () => {
        order.push("listen");
        return () => {};
      },
      listPendingShareOpens: async () => {
        order.push("list");
        return { status: "ok", data: [pendingId] };
      },
      handle,
    });

    expect(order).toEqual(["listen", "list", "handle"]);
    expect(handle).toHaveBeenCalledWith(pendingId);
    expect(unlisten).toEqual(expect.any(Function));
  });

  it("deduplicates an event also returned by the cold-start drain", async () => {
    const take = vi.fn().mockResolvedValue({
      status: "ok",
      data: { mode: "account", share_id: shareId },
    });
    const openAccount = vi.fn().mockResolvedValue(undefined);
    const processor = createShareOpenProcessor({
      takePendingShareOpen: take,
      getAuth: () => ({}) as any,
      openNew: vi.fn(),
      openAccount,
    });

    await Promise.all([
      processor.handle(pendingId),
      processor.handle(pendingId),
    ]);

    expect(take).toHaveBeenCalledTimes(1);
    expect(openAccount).toHaveBeenCalledTimes(1);
  });

  it("hydrates an account-authorized share before opening its durable tab", async () => {
    const abortSignal = new AbortController().signal;
    const abortSignalFn = vi.fn().mockResolvedValue({
      data: [{ share_id: shareId }],
      error: null,
    });
    const setHeader = vi.fn(() => ({ abortSignal: abortSignalFn }));
    const rpc = vi.fn(() => ({ setHeader }));
    const snapshot = { shareId };
    mocks.parseSnapshots.mockReturnValue([snapshot]);
    const openNew = vi.fn();

    await openAccountSharedNote({
      shareId,
      auth: {
        session: {
          token_type: "bearer",
          access_token: "account-token",
          user: { id: "viewer-1", is_anonymous: false },
        },
        supabase: { rpc },
      } as any,
      openNew,
      signal: abortSignal,
    });

    expect(rpc).toHaveBeenCalledWith("read_my_session_share_snapshot", {
      p_share_id: shareId,
    });
    expect(mocks.upsertCache).toHaveBeenCalledWith("viewer-1", snapshot);
    expect(openNew).toHaveBeenCalledWith({
      type: "shared_sessions",
      id: shareId,
    });
  });

  it("opens a handoff with a new local-only preview ID", () => {
    mocks.beginPreview.mockReturnValue(viewId);
    const openNew = vi.fn();

    openHandoffSharedNote({
      requestId,
      openNew,
    });

    expect(openNew).toHaveBeenCalledWith({
      type: "shared_note_preview",
      id: viewId,
    });
    expect(JSON.stringify(openNew.mock.calls)).not.toContain(requestId);
  });

  it("purges a claimed preview if its tab cannot open", () => {
    mocks.beginPreview.mockReturnValue(viewId);

    expect(() =>
      openHandoffSharedNote({
        requestId,
        openNew: () => {
          throw new Error("tab failed");
        },
      }),
    ).toThrow("shared-note preview unavailable");

    expect(mocks.purgePreview).toHaveBeenCalledWith(viewId);
  });
});
