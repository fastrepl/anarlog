import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => {
  const callbacks = new Map<
    string,
    {
      onData: (rows: unknown[]) => void;
      onError?: (error: string) => void;
    }
  >();
  const initialRows = new Map<string, unknown[]>();
  const unsubscribers = [vi.fn(), vi.fn(), vi.fn()];

  return {
    callbacks,
    initialRows,
    unsubscribers,
    subscribe: vi.fn(
      async (
        sql: string,
        _params: unknown[],
        options: {
          onData: (rows: unknown[]) => void;
          onError?: (error: string) => void;
        },
      ) => {
        const key = sql.includes("FROM sessions AS session")
          ? "sessions"
          : sql.includes("FROM humans")
            ? "humans"
            : "organizations";
        callbacks.set(key, options);
        options.onData(initialRows.get(key) ?? []);
        return unsubscribers[callbacks.size - 1];
      },
    ),
    reindex: vi.fn((_collection: string | null) =>
      Promise.resolve<
        { status: "ok"; data: null } | { status: "error"; error: string }
      >({ status: "ok", data: null }),
    ),
    updateDocuments: vi.fn((_documents: unknown[], _collection: null) =>
      Promise.resolve({ status: "ok", data: null }),
    ),
    removeDocument: vi.fn((_id: string, _collection: null) =>
      Promise.resolve({ status: "ok", data: null }),
    ),
  };
});

vi.mock("~/db", () => ({
  liveQueryClient: { subscribe: mocks.subscribe },
}));

vi.mock("@hypr/plugin-tantivy", () => ({
  commands: {
    reindex: mocks.reindex,
    updateDocuments: mocks.updateDocuments,
    removeDocument: mocks.removeDocument,
  },
}));

import { createSearchIndexSync } from "./indexing";

describe("SQLite search index synchronization", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.callbacks.clear();
    mocks.initialRows.clear();
    mocks.initialRows.set("sessions", [
      {
        id: "session-1",
        created_at: "2026-07-10T09:00:00.000Z",
        event_json: "",
        title: "Planning",
        raw_body: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "Raw note" }],
            },
          ],
        }),
        enhanced_notes_json: JSON.stringify(["Summary text"]),
        transcripts_json: JSON.stringify([
          JSON.stringify([{ text: "Spoken words" }]),
        ]),
      },
    ]);
    mocks.initialRows.set("humans", [
      {
        id: "human-1",
        name: "Alice",
        email: "alice@example.com",
        job_title: "Engineer",
        linkedin_username: "alice",
        created_at: "2026-07-10T08:00:00.000Z",
        memo: "Important contact",
      },
    ]);
    mocks.initialRows.set("organizations", [
      {
        id: "organization-1",
        name: "Acme",
        created_at: "2026-07-10T07:00:00.000Z",
      },
    ]);
    for (const unsubscribe of mocks.unsubscribers) {
      unsubscribe.mockResolvedValue(undefined);
    }
  });

  it("rebuilds stale index state from complete SQLite snapshots", async () => {
    const sync = createSearchIndexSync();

    await sync.start();

    expect(mocks.reindex).toHaveBeenCalledWith(null);
    expect(mocks.updateDocuments).toHaveBeenCalledTimes(1);
    const documents = mocks.updateDocuments.mock.calls[0][0];
    expect(documents).toHaveLength(3);
    expect(documents).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: "session-1",
          doc_type: "session",
          title: "Planning",
          content: "Raw note Summary text Spoken words",
        }),
        expect.objectContaining({
          id: "human-1",
          doc_type: "human",
          title: "Alice",
        }),
        expect.objectContaining({
          id: "organization-1",
          doc_type: "organization",
          title: "Acme",
        }),
      ]),
    );

    await sync.stop();
    for (const unsubscribe of mocks.unsubscribers) {
      expect(unsubscribe).toHaveBeenCalledOnce();
    }
  });

  it("updates changed rows and removes tombstoned rows", async () => {
    const sync = createSearchIndexSync();
    await sync.start();

    const session = mocks.initialRows.get("sessions")![0] as Record<
      string,
      unknown
    >;
    mocks.callbacks.get("sessions")!.onData([
      {
        ...session,
        title: "Updated planning",
      },
    ]);

    await vi.waitFor(() => {
      expect(mocks.updateDocuments).toHaveBeenCalledTimes(2);
    });
    expect(mocks.updateDocuments.mock.calls[1][0]).toEqual([
      expect.objectContaining({
        id: "session-1",
        title: "Updated planning",
      }),
    ]);

    mocks.callbacks.get("sessions")!.onData([]);

    await vi.waitFor(() => {
      expect(mocks.removeDocument).toHaveBeenCalledWith("session-1", null);
    });
    await sync.stop();
  });

  it("stops subscriptions when the Tantivy rebuild fails", async () => {
    mocks.reindex.mockResolvedValueOnce({
      status: "error",
      error: "index unavailable",
    });
    const sync = createSearchIndexSync();

    await expect(sync.start()).rejects.toThrow(
      "Failed to clear search index: index unavailable",
    );
    for (const unsubscribe of mocks.unsubscribers) {
      expect(unsubscribe).toHaveBeenCalledOnce();
    }
  });
});
