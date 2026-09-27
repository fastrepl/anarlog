import { createRequire } from "node:module";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(),
}));
vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  executeTransaction: mocks.executeTransaction,
}));
vi.mock("~/shared/utils", () => ({
  DEFAULT_USER_ID: "default-user",
  id: () => "new-event",
}));

import { syncSessionEmbeddedEvents } from "./process/events/execute";
import { eventTrackingIds } from "./process/events/identity";
import { syncEvents } from "./process/events/sync";
import {
  applyConnectionSync,
  loadEventsForSync,
  loadSessionsForTrackingIds,
} from "./storage";

import {
  LEGACY_MAIN_VALUES_ID,
  LEGACY_SETTINGS_ID,
} from "~/settings/legacy-snapshots";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");
let db: InstanceType<typeof DatabaseSync>;
const ctx = {
  provider: "apple" as const,
  connectionId: "apple",
  from: new Date("2026-09-14"),
  to: new Date("2026-09-20"),
  calendarIds: new Set(["calendar"]),
  calendarTrackingIdToId: new Map([["native-calendar", "calendar"]]),
};
const incoming = [
  {
    tracking_id_event: "apple:canonical-occurrence",
    tracking_id_calendar: "native-calendar",
    legacy_tracking_ids: ["store:uid:2026-09-15", "store:uid/RID=811159200"],
    title: "Rescheduled meeting",
    started_at: "2026-09-17T10:00:00Z",
    ended_at: "2026-09-17T11:00:00Z",
    has_recurrence_rules: false,
    recurrence_series_id: "uid",
    is_all_day: false,
  },
];

async function reconcile() {
  const ids = incoming.flatMap(eventTrackingIds);
  const existing = await loadEventsForSync(ctx, ids);
  const sessions = await loadSessionsForTrackingIds(ids);
  const events = syncEvents(ctx, {
    incoming,
    existing,
    incomingParticipants: new Map(),
  });
  await applyConnectionSync({
    ctx,
    events,
    sessionUpdates: syncSessionEmbeddedEvents(ctx, incoming, sessions),
    participants: {
      toAdd: [],
      toDelete: [],
      humansToCreate: [],
      humansToEnrich: [],
    },
  });
  return events;
}

function setting(id = "ignored_events") {
  return JSON.parse(
    (
      db
        .prepare("SELECT value_json FROM app_settings WHERE id = ?")
        .get(id) as { value_json: string }
    ).value_json,
  );
}

describe("Apple occurrence reconciliation in SQLite", () => {
  beforeEach(() => {
    db = new DatabaseSync(":memory:");
    db.exec(`
      CREATE TABLE events (
        id TEXT PRIMARY KEY, tracking_id_event TEXT, calendar_id TEXT DEFAULT 'calendar',
        title TEXT DEFAULT 'Original meeting', started_at TEXT DEFAULT '2026-09-15T10:00:00Z', ended_at TEXT DEFAULT '2026-09-15T11:00:00Z',
        location TEXT DEFAULT '', meeting_link TEXT DEFAULT '', description TEXT DEFAULT '', note TEXT DEFAULT '', recurrence_series_id TEXT DEFAULT 'uid',
        has_recurrence_rules INTEGER DEFAULT 1, is_all_day INTEGER DEFAULT 0, provider TEXT DEFAULT 'apple', participants_json TEXT DEFAULT '[]',
        created_at TEXT DEFAULT '2026-09-01', updated_at TEXT DEFAULT '2026-09-01', deleted_at TEXT
      );
      CREATE TABLE sessions (
        id TEXT PRIMARY KEY, owner_user_id TEXT DEFAULT 'user', event_id TEXT, event_json TEXT DEFAULT '{}', external_event_id TEXT DEFAULT '',
        external_provider TEXT DEFAULT 'apple', series_id TEXT DEFAULT 'uid', created_at TEXT DEFAULT '2026-09-01', updated_at TEXT DEFAULT '2026-09-01', deleted_at TEXT
      );
      CREATE TABLE app_settings (id TEXT PRIMARY KEY, value_json TEXT, updated_at TEXT);
      INSERT INTO events(id,tracking_id_event) VALUES ('row-original','store:uid:2026-09-15'), ('row-duplicate','store:uid/RID=811159200');
      INSERT INTO sessions(id,event_id) VALUES ('note-one','row-original'),('note-two','row-duplicate');
    `);
    db.prepare(
      "INSERT INTO app_settings VALUES ('ignored_events', ?, 'before')",
    ).run(
      JSON.stringify([
        { tracking_id: "store:uid:2026-09-15", last_seen: "2026-09-15" },
        { tracking_id: "other-event", last_seen: "2026-09-15" },
      ]),
    );
    mocks.execute.mockImplementation(async (sql, params) =>
      db.prepare(sql).all(...params),
    );
    mocks.executeTransaction.mockImplementation(async (statements) => {
      db.exec("BEGIN");
      try {
        const results = statements.map(
          ({ sql, params }: { sql: string; params: unknown[] }) =>
            db.prepare(sql).run(...(params as Array<string | number | null>))
              .changes,
        );
        db.exec("COMMIT");
        return results;
      } catch (error) {
        db.exec("ROLLBACK");
        throw error;
      }
    });
  });
  afterEach(() => db.close());

  test("retains a linked row, repairs both notes and ignores, and converges on repeat sync", async () => {
    // A linked row wins even when an unlinked duplicate is older.
    db.exec(
      "INSERT INTO events(id,tracking_id_event,created_at) VALUES ('older-unlinked','store:uid:2026-09-15','2026-08-01')",
    );
    const first = await reconcile();
    const kept = first.toUpdate[0].id;
    expect(["row-original", "row-duplicate"]).toContain(kept);
    expect(first.toAdd).toEqual([]);
    expect(
      db.prepare("SELECT id FROM events WHERE deleted_at IS NULL").all(),
    ).toEqual([{ id: kept }]);
    const notes = db
      .prepare(
        "SELECT event_id,external_event_id,event_json FROM sessions ORDER BY id",
      )
      .all();
    expect(notes).toHaveLength(2);
    for (const note of notes) {
      expect(note.event_id).toBe(kept);
      expect(note.external_event_id).toBe(incoming[0].tracking_id_event);
      expect(JSON.parse(String(note.event_json)).title).toBe(
        "Rescheduled meeting",
      );
    }
    expect(
      setting().map((entry: { tracking_id: string }) => entry.tracking_id),
    ).toEqual([incoming[0].tracking_id_event, "other-event"]);
    const second = await reconcile();
    expect(second.toAdd).toEqual([]);
    expect(second.toDelete).toEqual([]);
    expect(second.toUpdate[0].id).toBe(kept);
  });

  test("deduplicates ignored aliases and keeps the newest last_seen", async () => {
    db.prepare(
      "UPDATE app_settings SET value_json = ? WHERE id = 'ignored_events'",
    ).run(
      JSON.stringify([
        { tracking_id: incoming[0].tracking_id_event, last_seen: "2026-09-14" },
        {
          tracking_id: incoming[0].legacy_tracking_ids[0],
          last_seen: "2026-09-16",
        },
        {
          tracking_id: incoming[0].legacy_tracking_ids[1],
          last_seen: "2026-09-15",
        },
        { tracking_id: "other-event", last_seen: "2026-09-15" },
      ]),
    );
    await reconcile();
    const expected = [
      { tracking_id: incoming[0].tracking_id_event, last_seen: "2026-09-16" },
      { tracking_id: "other-event", last_seen: "2026-09-15" },
    ];
    expect(setting()).toEqual(expected);
    await reconcile();
    expect(setting()).toEqual(expected);
  });

  test("preserves ignored entries without tracking IDs during alias migration", async () => {
    const unidentified = [
      { last_seen: "2026-09-14", title: "Missing ID" },
      { last_seen: "2026-09-15", title: "Another missing ID" },
      { tracking_id: null, last_seen: "2026-09-16" },
      { tracking_id: null, last_seen: "2026-09-17" },
    ];
    db.prepare(
      "UPDATE app_settings SET value_json = ? WHERE id = 'ignored_events'",
    ).run(JSON.stringify([...unidentified, ...setting()]));

    await reconcile();
    const expected = [
      ...unidentified,
      { tracking_id: incoming[0].tracking_id_event, last_seen: "2026-09-15" },
      { tracking_id: "other-event", last_seen: "2026-09-15" },
    ];
    expect(setting()).toEqual(expected);
    await reconcile();
    expect(setting()).toEqual(expected);
  });

  test("rolls back event, note, and ignore changes together", async () => {
    db.exec(
      "CREATE TRIGGER reject_note BEFORE UPDATE ON sessions BEGIN SELECT RAISE(ABORT, 'test failure'); END",
    );
    await expect(reconcile()).rejects.toThrow("test failure");
    expect(
      db.prepare("SELECT id FROM events WHERE deleted_at IS NULL").all(),
    ).toHaveLength(2);
    expect(setting()[0].tracking_id).toBe("store:uid:2026-09-15");
    expect(
      db
        .prepare("SELECT external_event_id FROM sessions WHERE id='note-one'")
        .get(),
    ).toEqual({ external_event_id: "" });
  });

  test.each([LEGACY_MAIN_VALUES_ID, LEGACY_SETTINGS_ID])(
    "migrates ignored IDs from legacy snapshot %s",
    async (legacyId) => {
      const old = setting();
      db.exec("DELETE FROM app_settings");
      db.prepare("INSERT INTO app_settings VALUES (?,?,'before')").run(
        legacyId,
        JSON.stringify({ ignored_events: JSON.stringify(old) }),
      );
      await reconcile();
      expect(setting()[0].tracking_id).toBe(incoming[0].tracking_id_event);
      expect(setting(legacyId).ignored_events).toBe(JSON.stringify(old));
    },
  );

  test("an explicitly empty ignore list overrides legacy ignored entries", async () => {
    db.prepare("INSERT INTO app_settings VALUES (?,?,'before')").run(
      LEGACY_SETTINGS_ID,
      JSON.stringify({ ignored_events: setting() }),
    );
    db.exec(
      "UPDATE app_settings SET value_json='[]' WHERE id='ignored_events'",
    );
    await reconcile();
    expect(setting()).toEqual([]);
  });

  test("resurrects a linked legacy tombstone outside the visible window", async () => {
    db.exec(
      "UPDATE events SET deleted_at='2026-09-10',started_at='2026-08-15T10:00:00Z',ended_at='2026-08-15T11:00:00Z'",
    );
    const result = await reconcile();
    expect(result.toUpdate).toHaveLength(1);
    expect(result.toAdd).toEqual([]);
    expect(
      db.prepare("SELECT id FROM events WHERE deleted_at IS NULL").all(),
    ).toHaveLength(1);
    expect(
      db.prepare("SELECT DISTINCT event_id FROM sessions").all(),
    ).toHaveLength(1);
  });
});
