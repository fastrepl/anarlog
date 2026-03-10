import { describe, expect, test } from "vitest";

import type { Ctx } from "../../ctx";
import { syncSessionParticipants } from "./sync";

type MockStoreData = {
  humans: Record<string, { email?: string; name?: string }>;
  sessions: Record<string, { event_json?: string }>;
  events: Record<string, { tracking_id_event?: string }>;
  mapping_session_participant: Record<
    string,
    { session_id: string; human_id: string; source?: string }
  >;
};

function createMockStore(data: MockStoreData) {
  return {
    getRow: (table: keyof MockStoreData, id: string) => data[table]?.[id] ?? {},
    hasRow: (table: keyof MockStoreData, id: string) => !!data[table]?.[id],
    forEachRow: (
      table: keyof MockStoreData,
      callback: (id: string, forEachCell: unknown) => void,
    ) => {
      const tableData = data[table] ?? {};
      for (const id of Object.keys(tableData)) {
        callback(id, () => {});
      }
    },
  } as unknown as Ctx["store"];
}

function createMockCtx(store: Ctx["store"]): Ctx {
  return {
    store,
    provider: "apple" as const,
    userId: "user-1",
    from: new Date("2024-01-01"),
    to: new Date("2024-02-01"),
    calendarIds: new Set(["cal-1"]),
    calendarTrackingIdToId: new Map([["tracking-cal-1", "cal-1"]]),
  };
}

describe("syncParticipants", () => {
  test("returns empty output when no events provided", () => {
    const store = createMockStore({
      humans: {},
      sessions: {},
      events: {},
      mapping_session_participant: {},
    });
    const ctx = createMockCtx(store);

    const result = syncSessionParticipants(ctx, {
      incomingParticipants: new Map(),
    });

    expect(result.toAdd).toHaveLength(0);
    expect(result.toDelete).toHaveLength(0);
    expect(result.humansToCreate).toHaveLength(0);
  });

  test("skips events without associated session", () => {
    const store = createMockStore({
      humans: {},
      sessions: {},
      events: {},
      mapping_session_participant: {},
    });
    const ctx = createMockCtx(store);

    const result = syncSessionParticipants(ctx, {
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "test@example.com", name: "Test" }]],
      ]),
    });

    expect(result.toAdd).toHaveLength(0);
    expect(result.humansToCreate).toHaveLength(0);
  });

  test("creates new human when participant email not found", () => {
    const store = createMockStore({
      humans: {},
      sessions: {
        "session-1": {
          event_json: JSON.stringify({ tracking_id: "tracking-1" }),
        },
      },
      events: { "event-1": { tracking_id_event: "tracking-1" } },
      mapping_session_participant: {},
    });
    const ctx = createMockCtx(store);

    const result = syncSessionParticipants(ctx, {
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "new@example.com", name: "New Person" }]],
      ]),
    });

    expect(result.humansToCreate).toHaveLength(1);
    expect(result.humansToCreate[0].email).toBe("new@example.com");
    expect(result.humansToCreate[0].name).toBe("New Person");
  });

  test("uses existing human when email matches", () => {
    const store = createMockStore({
      humans: { "human-1": { email: "existing@example.com" } },
      sessions: {
        "session-1": {
          event_json: JSON.stringify({ tracking_id: "tracking-1" }),
        },
      },
      events: { "event-1": { tracking_id_event: "tracking-1" } },
      mapping_session_participant: {},
    });
    const ctx = createMockCtx(store);

    const result = syncSessionParticipants(ctx, {
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "existing@example.com", name: "Existing" }]],
      ]),
    });

    expect(result.humansToCreate).toHaveLength(0);
    expect(result.toAdd).toHaveLength(1);
    expect(result.toAdd[0].humanId).toBe("human-1");
  });

  test("deletes auto-source mappings when participant removed from event", () => {
    const store = createMockStore({
      humans: { "human-1": { email: "removed@example.com" } },
      sessions: {
        "session-1": {
          event_json: JSON.stringify({ tracking_id: "tracking-1" }),
        },
      },
      events: { "event-1": { tracking_id_event: "tracking-1" } },
      mapping_session_participant: {
        "mapping-1": {
          session_id: "session-1",
          human_id: "human-1",
          source: "auto",
        },
      },
    });
    const ctx = createMockCtx(store);

    const result = syncSessionParticipants(ctx, {
      incomingParticipants: new Map([["tracking-1", []]]),
    });

    expect(result.toDelete).toContain("mapping-1");
  });

  test("does not delete excluded mappings", () => {
    const store = createMockStore({
      humans: { "human-1": { email: "excluded@example.com" } },
      sessions: {
        "session-1": {
          event_json: JSON.stringify({ tracking_id: "tracking-1" }),
        },
      },
      events: {},
      mapping_session_participant: {
        "mapping-1": {
          session_id: "session-1",
          human_id: "human-1",
          source: "excluded",
        },
      },
    });
    const ctx = createMockCtx(store);

    const result = syncSessionParticipants(ctx, {
      incomingParticipants: new Map([["tracking-1", []]]),
    });

    expect(result.toDelete).not.toContain("mapping-1");
  });
});
