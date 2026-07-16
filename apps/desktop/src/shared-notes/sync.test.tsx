import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  auth: {
    session: null as any,
    supabase: null as any,
  },
  abortSignal: vi.fn(),
  invalidateResource: vi.fn(),
  history: new Map<string, { stack: Array<{ type: string; id: string }> }>(),
  parseSnapshots: vi.fn((value: unknown) => value),
  replaceCache: vi.fn(async () => {}),
  rpc: vi.fn(),
  setHeader: vi.fn(),
  tabs: [] as Array<{ type: string; id: string }>,
}));

vi.mock("~/auth", () => ({
  useAuth: () => mocks.auth,
}));

vi.mock("./cache", () => ({
  parseDurableSharedNoteSnapshots: mocks.parseSnapshots,
  replaceDurableSharedNoteCache: mocks.replaceCache,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => ({
      history: mocks.history,
      tabs: mocks.tabs,
      invalidateResource: mocks.invalidateResource,
    }),
  },
}));

import { DurableSharedNoteCacheSync, syncDurableSharedNoteCache } from "./sync";

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

describe("DurableSharedNoteCacheSync", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.auth.session = {
      access_token: "access-token",
      expires_at: 1_800_000_000,
      token_type: "bearer",
      user: { id: "viewer-1", is_anonymous: false },
    };
    mocks.auth.supabase = { rpc: mocks.rpc };
    mocks.rpc.mockReturnValue({ setHeader: mocks.setHeader });
    mocks.setHeader.mockReturnValue({ abortSignal: mocks.abortSignal });
    mocks.abortSignal.mockResolvedValue({ data: [], error: null });
    mocks.parseSnapshots.mockImplementation((value) => value);
    mocks.history = new Map();
    mocks.tabs = [];
  });

  afterEach(cleanup);

  it("closes durable shared-note tabs removed by reconciliation", async () => {
    mocks.tabs = [
      { type: "shared_sessions", id: "revoked-share" },
      { type: "shared_sessions", id: "active-share" },
      { type: "sessions", id: "personal-note" },
    ];
    mocks.history = new Map([
      [
        "slot-1",
        {
          stack: [{ type: "shared_sessions", id: "revoked-history-share" }],
        },
      ],
    ]);
    mocks.parseSnapshots.mockReturnValue([
      { shareId: "active-share", accessVersion: 1 },
    ]);

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.invalidateResource).toHaveBeenCalledWith(
        "shared_sessions",
        "revoked-share",
      );
    });
    expect(mocks.invalidateResource).not.toHaveBeenCalledWith(
      "shared_sessions",
      "active-share",
    );
    expect(mocks.invalidateResource).toHaveBeenCalledWith(
      "shared_sessions",
      "revoked-history-share",
    );
  });

  it("reconciles a successful authenticated RPC response", async () => {
    const snapshots = [
      {
        shareId: "share-1",
        accessVersion: 3,
      },
    ];
    mocks.abortSignal.mockResolvedValue({ data: ["server-row"], error: null });
    mocks.parseSnapshots.mockReturnValue(snapshots);

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.replaceCache).toHaveBeenCalledWith("viewer-1", snapshots);
    });
    expect(mocks.rpc).toHaveBeenCalledWith(
      "list_my_session_share_snapshot_page",
      { p_after_share_id: null, p_limit: 100 },
    );
    expect(mocks.setHeader).toHaveBeenCalledWith(
      "Authorization",
      "bearer access-token",
    );
    expect(mocks.abortSignal).toHaveBeenCalledWith(expect.any(AbortSignal));
  });

  it("reconciles only after every result page succeeds", async () => {
    const firstPage = Array.from({ length: 100 }, (_, index) => ({
      share_id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
    }));
    const lastPage = [{ share_id: "00000000-0000-4000-8000-000000000100" }];
    mocks.abortSignal
      .mockResolvedValueOnce({ data: firstPage, error: null })
      .mockResolvedValueOnce({ data: lastPage, error: null });
    mocks.parseSnapshots.mockReturnValue([]);

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.replaceCache).toHaveBeenCalledWith("viewer-1", []);
    });
    expect(mocks.rpc).toHaveBeenNthCalledWith(
      1,
      "list_my_session_share_snapshot_page",
      { p_after_share_id: null, p_limit: 100 },
    );
    expect(mocks.rpc).toHaveBeenNthCalledWith(
      2,
      "list_my_session_share_snapshot_page",
      {
        p_after_share_id: firstPage[firstPage.length - 1]?.share_id,
        p_limit: 100,
      },
    );
    expect(mocks.parseSnapshots).toHaveBeenCalledWith([
      ...firstPage,
      ...lastPage,
    ]);
  });

  it("preserves the cache when a later result page fails", async () => {
    mocks.abortSignal
      .mockResolvedValueOnce({
        data: Array.from({ length: 100 }, (_, index) => ({
          share_id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
        })),
        error: null,
      })
      .mockResolvedValueOnce({
        data: null,
        error: new Error("page unavailable"),
      });

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.abortSignal).toHaveBeenCalledTimes(2);
    });
    expect(mocks.parseSnapshots).not.toHaveBeenCalled();
    expect(mocks.replaceCache).not.toHaveBeenCalled();
  });

  it("rejects a full page that does not advance the share cursor", async () => {
    const page = Array.from({ length: 100 }, (_, index) => ({
      share_id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
    }));
    mocks.abortSignal
      .mockResolvedValueOnce({ data: page, error: null })
      .mockResolvedValueOnce({ data: page, error: null });

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.abortSignal).toHaveBeenCalledTimes(2);
    });
    expect(mocks.parseSnapshots).not.toHaveBeenCalled();
    expect(mocks.replaceCache).not.toHaveBeenCalled();
    expect(mocks.invalidateResource).not.toHaveBeenCalled();
  });

  it("does not invalidate tabs when the admitted session is aborted", async () => {
    let finishCacheWrite: (() => void) | undefined;
    mocks.replaceCache.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finishCacheWrite = resolve;
        }),
    );
    mocks.parseSnapshots.mockReturnValue([
      { shareId: "active-share", accessVersion: 1 },
    ]);
    mocks.tabs = [{ type: "shared_sessions", id: "other-share" }];
    const controller = new AbortController();
    const reconciliation = syncDurableSharedNoteCache(
      mocks.auth.supabase,
      mocks.auth.session,
      controller.signal,
    );

    await waitFor(() => {
      expect(mocks.replaceCache).toHaveBeenCalled();
    });
    controller.abort();
    finishCacheWrite?.();

    await expect(reconciliation).rejects.toMatchObject({ name: "AbortError" });
    expect(mocks.invalidateResource).not.toHaveBeenCalled();
  });

  it("does not query while signed out or using an anonymous session", async () => {
    mocks.auth.session = null;
    const signedOut = render(<DurableSharedNoteCacheSync />, {
      wrapper: createWrapper(),
    });

    await Promise.resolve();
    expect(mocks.rpc).not.toHaveBeenCalled();
    signedOut.unmount();

    mocks.auth.session = {
      access_token: "anonymous-token",
      user: { id: "anonymous-1", is_anonymous: true },
    };
    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await Promise.resolve();
    expect(mocks.rpc).not.toHaveBeenCalled();
  });

  it("preserves the local cache when the RPC fails", async () => {
    mocks.abortSignal.mockResolvedValue({
      data: null,
      error: new Error("unavailable"),
    });

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.abortSignal).toHaveBeenCalled();
    });
    expect(mocks.parseSnapshots).not.toHaveBeenCalled();
    expect(mocks.replaceCache).not.toHaveBeenCalled();
  });

  it("preserves the local cache when payload validation fails", async () => {
    mocks.parseSnapshots.mockImplementation(() => {
      throw new Error("invalid snapshot");
    });

    render(<DurableSharedNoteCacheSync />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(mocks.parseSnapshots).toHaveBeenCalled();
    });
    expect(mocks.replaceCache).not.toHaveBeenCalled();
  });
});
