import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  auth: {
    session: null as any,
  },
  durableNotes: [] as any[],
  sourceRevisions: [] as any[],
  loadSessionShareSource: vi.fn(),
  publishSessionShareSnapshot: vi.fn(),
  upsertDurableSharedNoteCache: vi.fn(async () => {}),
}));

vi.mock("~/auth", () => ({
  useAuth: () => mocks.auth,
}));

vi.mock("~/db", () => ({
  useLiveQuery: () => ({ data: mocks.sourceRevisions }),
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

vi.mock("./source", () => ({
  loadSessionShareSource: mocks.loadSessionShareSource,
}));

import { OwnedSharedNotePublisher, shouldPublishOwnedShare } from "./sync";

const OWNER_ID = "11111111-1111-4111-8111-111111111111";
const WORKSPACE_ID = "22222222-2222-4222-8222-222222222222";
const SHARE_ID = "33333333-3333-4333-8333-333333333333";

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
        capability: "editor",
        manageAccess: true,
        accessVersion: 3,
        publishedAt: "2026-07-17T09:00:00.000Z",
      },
    ];
    mocks.loadSessionShareSource.mockResolvedValue({
      workspaceId: WORKSPACE_ID,
      sessionId: "session-1",
      title: "Current title",
      body: { type: "doc", content: [{ type: "paragraph" }] },
    });
    mocks.publishSessionShareSnapshot.mockResolvedValue({
      ...mocks.durableNotes[0],
      contentRevision: 3,
      title: "Current title",
      body: { type: "doc", content: [{ type: "paragraph" }] },
      publishedAt: "2026-07-17T09:02:00.000Z",
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("publishes a newer managed note after the debounce and refreshes its durable snapshot", async () => {
    const { queryClient } = renderPublisher();

    expect(
      queryClient
        .getQueryCache()
        .getAll()
        .map((query) => query.queryKey),
    ).toEqual([
      [
        "owned-shared-note-publish",
        OWNER_ID,
        SHARE_ID,
        "2026-07-17T09:01:00.000Z",
      ],
    ]);
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.loadSessionShareSource).toHaveBeenCalledWith(
      "session-1",
      OWNER_ID,
    );
    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledWith(
      expect.objectContaining({
        apiBaseUrl: "https://api.example.com",
        session: mocks.auth.session,
        shareId: SHARE_ID,
        title: "Current title",
        signal: expect.any(AbortSignal),
      }),
    );
    expect(mocks.upsertDurableSharedNoteCache).toHaveBeenCalledWith(
      OWNER_ID,
      expect.objectContaining({
        shareId: SHARE_ID,
        contentRevision: 3,
        title: "Current title",
        publishedAt: "2026-07-17T09:02:00.000Z",
      }),
    );
  });

  it("aborts the pending debounce when the publisher unmounts", async () => {
    const view = renderPublisher();

    view.unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
  });

  it("does not publish from an anonymous account", async () => {
    mocks.auth.session = {
      access_token: "anonymous-access-token",
      token_type: "bearer",
      user: { id: OWNER_ID, is_anonymous: true },
    };

    renderPublisher();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
  });
});

describe("shouldPublishOwnedShare", () => {
  it("publishes only when the valid local revision is newer", () => {
    expect(
      shouldPublishOwnedShare(
        "2026-07-17T09:01:00.000Z",
        "2026-07-17T09:00:00.000Z",
      ),
    ).toBe(true);
    expect(
      shouldPublishOwnedShare(
        "2026-07-17T09:00:00.000Z",
        "2026-07-17T09:00:00.000Z",
      ),
    ).toBe(false);
    expect(shouldPublishOwnedShare("invalid", "also-invalid")).toBe(false);
  });
});
