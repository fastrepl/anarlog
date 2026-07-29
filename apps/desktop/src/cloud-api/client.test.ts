import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  fetch: vi.fn(),
  getSession: vi.fn(),
  refreshSession: vi.fn(),
  getCloudSnapshot: vi.fn(),
  listCloudSnapshotIds: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: mocks.fetch }));
vi.mock("@anlg/plugin-local-api", () => ({
  commands: {
    getCloudSnapshot: mocks.getCloudSnapshot,
    listCloudSnapshotIds: mocks.listCloudSnapshotIds,
  },
}));
vi.mock("~/auth/client", () => ({
  supabase: {
    auth: {
      getSession: mocks.getSession,
      refreshSession: mocks.refreshSession,
    },
  },
}));
vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.anarlog.test" },
}));

const storedValues = new Map<string, string>();
vi.stubGlobal("localStorage", {
  clear: () => storedValues.clear(),
  getItem: (key: string) => storedValues.get(key) ?? null,
  removeItem: (key: string) => storedValues.delete(key),
  setItem: (key: string, value: string) => storedValues.set(key, value),
});

import {
  backfillCloudApiSnapshots,
  CloudApiClientError,
  deleteCloudApiSnapshotBestEffort,
  initializeCloudApiBackfill,
  scheduleCloudApiSnapshotSync,
  syncCloudApiSnapshot,
  syncCloudApiSnapshotBestEffort,
} from "./client";

function authSession(userId: string) {
  return {
    data: {
      session: {
        access_token: `token-${userId}`,
        user: { id: userId, is_anonymous: false },
      },
    },
    error: null,
  };
}

describe("cloud API client", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mocks.fetch.mockResolvedValue(
      new Response(JSON.stringify({ enabled: true, updated_at: null }), {
        status: 200,
      }),
    );
    mocks.getCloudSnapshot.mockResolvedValue({
      status: "ok",
      data: { id: "meeting-1", transcripts: [] },
    });
    mocks.listCloudSnapshotIds.mockResolvedValue({
      status: "ok",
      data: [],
    });
  });

  it("does not upload a local snapshot after the signed-in account changes", async () => {
    mocks.getSession
      .mockResolvedValueOnce(authSession("user-a"))
      .mockResolvedValueOnce(authSession("user-a"))
      .mockResolvedValueOnce(authSession("user-a"))
      .mockResolvedValueOnce(authSession("user-a"))
      .mockResolvedValueOnce(authSession("user-b"));

    await expect(syncCloudApiSnapshot("meeting-1")).rejects.toEqual(
      expect.objectContaining<Partial<CloudApiClientError>>({
        code: "unauthorized",
      }),
    );

    expect(mocks.fetch).toHaveBeenCalledTimes(1);
    expect(mocks.getCloudSnapshot).toHaveBeenCalledWith("meeting-1");
  });

  it("continues backfill after a meeting cannot be uploaded", async () => {
    mocks.getSession.mockResolvedValue(authSession("user-backfill"));
    mocks.listCloudSnapshotIds.mockResolvedValue({
      status: "ok",
      data: ["meeting-too-large", "meeting-2"],
    });
    mocks.getCloudSnapshot.mockImplementation(async (sessionId: string) => ({
      status: "ok",
      data: { id: sessionId, transcripts: [] },
    }));
    mocks.fetch.mockImplementation(async (input: string | URL | Request) => {
      const url = String(input);
      if (url.endsWith("/v1/cloud-api/settings")) {
        return new Response(
          JSON.stringify({ enabled: true, updated_at: null }),
          { status: 200 },
        );
      }
      if (url.includes("meeting-too-large")) {
        return new Response(
          JSON.stringify({
            error: { code: "invalid_request", message: "Snapshot too large" },
          }),
          { status: 400 },
        );
      }
      return new Response(null, { status: 204 });
    });
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    await expect(backfillCloudApiSnapshots()).resolves.toBe(1);

    expect(mocks.getCloudSnapshot).toHaveBeenCalledWith("meeting-too-large");
    expect(mocks.getCloudSnapshot).toHaveBeenCalledWith("meeting-2");
    expect(consoleError).toHaveBeenCalledOnce();
  });

  it("queues transient local snapshot failures for retry", async () => {
    mocks.getSession.mockResolvedValue(authSession("user-local-snapshot"));
    mocks.listCloudSnapshotIds.mockResolvedValue({
      status: "ok",
      data: ["meeting-locked"],
    });
    mocks.getCloudSnapshot.mockResolvedValue({
      status: "error",
      error: "database is locked",
    });
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    await expect(backfillCloudApiSnapshots()).rejects.toEqual(
      expect.objectContaining<Partial<CloudApiClientError>>({
        code: "local_snapshot_unavailable",
      }),
    );

    expect(
      JSON.parse(
        localStorage.getItem(
          "anarlog.cloud-api-pending.v1.user-local-snapshot",
        ) ?? "{}",
      ),
    ).toEqual({
      upserts: ["meeting-locked"],
      deletes: [],
    });
    expect(consoleError).toHaveBeenCalledOnce();
  });

  it("keeps deletion dominant over a stale failed upload", async () => {
    mocks.getSession.mockResolvedValue(authSession("user-delete-race"));
    mocks.listCloudSnapshotIds.mockResolvedValue({
      status: "ok",
      data: ["meeting-deleted"],
    });
    let resolveSnapshot:
      | ((result: { status: "error"; error: string }) => void)
      | undefined;
    mocks.getCloudSnapshot.mockReturnValue(
      new Promise((resolve) => {
        resolveSnapshot = resolve;
      }),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    syncCloudApiSnapshotBestEffort("meeting-deleted");
    await vi.waitFor(() =>
      expect(mocks.getCloudSnapshot).toHaveBeenCalledWith("meeting-deleted"),
    );

    deleteCloudApiSnapshotBestEffort("meeting-deleted");
    await vi.waitFor(() =>
      expect(
        JSON.parse(
          localStorage.getItem(
            "anarlog.cloud-api-pending.v1.user-delete-race",
          ) ?? "{}",
        ).deletes,
      ).toEqual(["meeting-deleted"]),
    );
    expect(
      mocks.fetch.mock.calls.some(
        ([, init]) => (init as RequestInit | undefined)?.method === "DELETE",
      ),
    ).toBe(false);

    resolveSnapshot?.({ status: "error", error: "database is locked" });
    await vi.waitFor(() =>
      expect(mocks.fetch).toHaveBeenCalledWith(
        expect.stringContaining("/v1/sync-snapshots/meeting-deleted"),
        expect.objectContaining({ method: "DELETE" }),
      ),
    );

    await vi.waitFor(() => expect(consoleError).toHaveBeenCalledOnce());

    expect(
      JSON.parse(
        localStorage.getItem("anarlog.cloud-api-pending.v1.user-delete-race") ??
          "{}",
      ),
    ).toEqual({
      upserts: [],
      deletes: [],
    });
  });

  it("runs a delete after every upload already queued for the meeting", async () => {
    mocks.getSession.mockResolvedValue(authSession("user-delete-order"));
    const resolveSnapshots: Array<
      (result: {
        status: "ok";
        data: { id: string; transcripts: never[] };
      }) => void
    > = [];
    mocks.getCloudSnapshot.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveSnapshots.push(resolve);
        }),
    );

    const firstSync = syncCloudApiSnapshot("meeting-queued");
    const secondSync = syncCloudApiSnapshot("meeting-queued");
    await vi.waitFor(() =>
      expect(mocks.getCloudSnapshot).toHaveBeenCalledOnce(),
    );

    deleteCloudApiSnapshotBestEffort("meeting-queued");
    resolveSnapshots[0]?.({
      status: "ok",
      data: { id: "meeting-queued", transcripts: [] },
    });
    await vi.waitFor(() =>
      expect(mocks.getCloudSnapshot).toHaveBeenCalledTimes(2),
    );
    resolveSnapshots[1]?.({
      status: "ok",
      data: { id: "meeting-queued", transcripts: [] },
    });

    await Promise.all([firstSync, secondSync]);
    await vi.waitFor(() =>
      expect(mocks.fetch).toHaveBeenCalledWith(
        expect.stringContaining("/v1/sync-snapshots/meeting-queued"),
        expect.objectContaining({ method: "DELETE" }),
      ),
    );
    expect(
      mocks.fetch.mock.calls
        .filter(([url]) => String(url).includes("/v1/sync-snapshots/"))
        .map(([, init]) => (init as RequestInit).method),
    ).toEqual(["DELETE"]);
  });

  it("cancels a scheduled upload when the meeting is deleted", async () => {
    vi.useFakeTimers();
    mocks.getSession.mockResolvedValue(authSession("user-timer-delete"));

    scheduleCloudApiSnapshotSync("meeting-scheduled");
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(1);

    deleteCloudApiSnapshotBestEffort("meeting-scheduled");
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(1500);

    expect(mocks.getCloudSnapshot).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("deletes a pending snapshot after its local meeting disappears", async () => {
    mocks.getSession.mockResolvedValue(authSession("user-gone"));
    mocks.getCloudSnapshot.mockResolvedValue({
      status: "error",
      error: "meeting not found",
    });
    mocks.listCloudSnapshotIds.mockResolvedValue({
      status: "ok",
      data: [],
    });
    localStorage.setItem("anarlog.cloud-api-backfill.v1.user-gone", "complete");
    localStorage.setItem(
      "anarlog.cloud-api-pending.v1.user-gone",
      JSON.stringify({ upserts: ["meeting-gone"], deletes: [] }),
    );

    await initializeCloudApiBackfill();

    expect(mocks.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/v1/sync-snapshots/meeting-gone"),
      expect.objectContaining({ method: "DELETE" }),
    );
    expect(
      JSON.parse(
        localStorage.getItem("anarlog.cloud-api-pending.v1.user-gone") ?? "{}",
      ),
    ).toEqual({
      upserts: [],
      deletes: [],
    });
  });
});
