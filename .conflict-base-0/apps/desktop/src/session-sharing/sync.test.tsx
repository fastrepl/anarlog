import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  auth: { session: null as any },
  durableNotes: [] as any[],
  sourceRevisions: [] as any[],
  liveQueryOptions: null as any,
  createMutationId: vi.fn().mockResolvedValue("mutation-id"),
  hashProjection: vi.fn(),
  loadProjection: vi.fn(),
  loadState: vi.fn(),
  publishSessionShareSnapshot: vi.fn(),
  recordState: vi.fn(async () => {}),
  upsertDurableSharedNoteCache: vi.fn(async () => {}),
}));

vi.mock("~/auth", () => ({ useAuth: () => mocks.auth }));
vi.mock("~/db", () => ({
  useLiveQuery: (options: unknown) => {
    mocks.liveQueryOptions = options;
    return { data: mocks.sourceRevisions };
  },
}));
vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.example.com" },
}));
vi.mock("~/shared-notes/cache", () => ({
  useDurableSharedNotes: () => mocks.durableNotes,
  upsertDurableSharedNoteCache: mocks.upsertDurableSharedNoteCache,
}));
vi.mock("./client", () => ({
  publishSessionShareSnapshot: mocks.publishSessionShareSnapshot,
}));
vi.mock("./reconciliation", () => ({
  createSessionShareMutationId: mocks.createMutationId,
  hashSessionShareProjection: mocks.hashProjection,
  loadManagedShareProjection: mocks.loadProjection,
  loadSessionShareSyncState: mocks.loadState,
  recordPublishedSessionShareState: mocks.recordState,
}));

import { OwnedSharedNotePublisher } from "./sync";

const OWNER_ID = "11111111-1111-4111-8111-111111111111";
const WORKSPACE_ID = "22222222-2222-4222-8222-222222222222";
const SHARE_ID = "33333333-3333-4333-8333-333333333333";
const BASELINE_HASH = "a".repeat(64);
const CURRENT_HASH = "b".repeat(64);

function renderPublisher() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const view = render(
    <QueryClientProvider client={queryClient}>
      <OwnedSharedNotePublisher />
    </QueryClientProvider>,
  );
  return { ...view, queryClient };
}

describe("OwnedSharedNotePublisher", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    mocks.auth.session = {
      access_token: "owner-access-token",
      token_type: "bearer",
      user: { id: OWNER_ID, is_anonymous: false },
    };
    mocks.sourceRevisions = [
      {
        shareId: SHARE_ID,
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        sourceUpdatedAt: "2026-07-17T09:01:00.000Z",
        acknowledgedContentRevision: 2,
        baselineSourceHash: BASELINE_HASH,
        syncStatus: "clean",
      },
    ];
    mocks.durableNotes = [
      {
        shareId: SHARE_ID,
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        schemaVersion: 1,
        contentRevision: 2,
        title: "Earlier title",
        body: { type: "doc", content: [] },
        attachments: [],
        capability: "editor",
        manageAccess: true,
        accessVersion: 3,
        webEditable: true,
        webEditBase: null,
        publishedAt: "2026-07-17T09:00:00.000Z",
      },
    ];
    mocks.loadProjection.mockResolvedValue({
      source: {
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        title: "Current title",
      },
      body: { type: "doc", content: [{ type: "paragraph" }] },
      hash: CURRENT_HASH,
    });
    mocks.loadState.mockResolvedValue({
      viewerUserId: OWNER_ID,
      shareId: SHARE_ID,
      sessionId: "session-1",
      acknowledgedContentRevision: 2,
      baselineSourceHash: BASELINE_HASH,
      status: "clean",
    });
    mocks.hashProjection.mockResolvedValue(BASELINE_HASH);
    mocks.publishSessionShareSnapshot.mockResolvedValue({
      shareId: SHARE_ID,
      schemaVersion: 1,
      contentRevision: 3,
      title: "Current title",
      body: { type: "doc", content: [{ type: "paragraph" }] },
      attachments: [],
      accessVersion: 3,
      webEditable: true,
      publishedAt: "2026-07-17T09:02:00.000Z",
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("publishes only a local change based on the acknowledged server revision", async () => {
    renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledWith(
      expect.objectContaining({
        shareId: SHARE_ID,
        baseRevision: 2,
        mutationId: "mutation-id",
        title: "Current title",
      }),
    );
    expect(mocks.createMutationId).toHaveBeenCalledWith({
      shareId: SHARE_ID,
      baseRevision: 2,
      sourceHash: CURRENT_HASH,
      attachmentIds: [],
    });
    expect(mocks.recordState).toHaveBeenCalledWith(
      expect.objectContaining({
        contentRevision: 3,
        sourceHash: CURRENT_HASH,
      }),
    );
    expect(mocks.upsertDurableSharedNoteCache).toHaveBeenCalledWith(
      OWNER_ID,
      expect.objectContaining({ contentRevision: 3, webEditBase: null }),
    );
  });

  it("observes the canonical note or the latest legacy fallback document", () => {
    renderPublisher();

    const sql = mocks.liveQueryOptions.sql.replace(/\s+/g, " ");
    expect(sql).toContain("canonical.id = session.id");
    expect(sql).toContain("fallback.session_id = session.id");
    expect(sql).toContain(
      "ORDER BY fallback.updated_at DESC, fallback.created_at DESC, fallback.id",
    );
  });

  it("does not echo an imported remote body when its hash is the baseline", async () => {
    mocks.loadProjection.mockResolvedValue({
      source: {
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        title: "Imported title",
      },
      body: { type: "doc" },
      hash: BASELINE_HASH,
    });
    renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
  });

  it.each([
    { syncStatus: "conflict", webEditBase: null },
    {
      syncStatus: "clean",
      webEditBase: {
        contentRevision: 1,
        title: "Base",
        body: { type: "doc" },
      },
    },
  ])("does not publish a pending or conflicting note", async (override) => {
    mocks.sourceRevisions[0].syncStatus = override.syncStatus;
    mocks.durableNotes[0].webEditBase = override.webEditBase;
    renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.loadProjection).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
  });

  it("does not assess a legacy snapshot whose durable copy differs locally", async () => {
    mocks.sourceRevisions[0] = {
      ...mocks.sourceRevisions[0],
      acknowledgedContentRevision: null,
      baselineSourceHash: null,
      syncStatus: null,
    };
    mocks.durableNotes[0].webEditable = false;
    mocks.loadState.mockResolvedValue(null);
    mocks.hashProjection.mockResolvedValue(BASELINE_HASH);
    renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.loadProjection).toHaveBeenCalledOnce();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.recordState).not.toHaveBeenCalled();
  });

  it("performs one assessment publish for an exact legacy snapshot", async () => {
    mocks.sourceRevisions[0] = {
      ...mocks.sourceRevisions[0],
      acknowledgedContentRevision: null,
      baselineSourceHash: null,
      syncStatus: null,
    };
    mocks.durableNotes[0].webEditable = false;
    mocks.loadState.mockResolvedValue(null);
    mocks.loadProjection.mockResolvedValue({
      source: {
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        title: "Earlier title",
      },
      body: { type: "doc", content: [] },
      hash: BASELINE_HASH,
    });
    renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledOnce();
    expect(mocks.recordState).toHaveBeenCalledOnce();
  });

  it("does not repeat an assessment when the server keeps web editing disabled", async () => {
    mocks.sourceRevisions[0] = {
      ...mocks.sourceRevisions[0],
      acknowledgedContentRevision: null,
      baselineSourceHash: null,
      syncStatus: null,
    };
    mocks.durableNotes[0].webEditable = false;
    mocks.loadState.mockResolvedValue(null);
    mocks.loadProjection.mockResolvedValue({
      source: {
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        title: "Earlier title",
      },
      body: { type: "doc", content: [] },
      hash: BASELINE_HASH,
    });
    mocks.publishSessionShareSnapshot.mockResolvedValueOnce({
      ...mocks.durableNotes[0],
      contentRevision: 3,
      webEditable: false,
    });
    const first = renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledOnce();
    first.unmount();
    mocks.sourceRevisions[0] = {
      ...mocks.sourceRevisions[0],
      acknowledgedContentRevision: 3,
      baselineSourceHash: BASELINE_HASH,
      syncStatus: "clean",
    };
    mocks.durableNotes[0] = {
      ...mocks.durableNotes[0],
      contentRevision: 3,
      webEditable: false,
    };
    mocks.loadState.mockResolvedValue({
      viewerUserId: OWNER_ID,
      shareId: SHARE_ID,
      sessionId: "session-1",
      acknowledgedContentRevision: 3,
      baselineSourceHash: BASELINE_HASH,
      status: "clean",
    });
    renderPublisher();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledOnce();
  });

  it("aborts the pending debounce when the publisher unmounts", async () => {
    const view = renderPublisher();
    view.unmount();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.loadProjection).not.toHaveBeenCalled();
  });
});
