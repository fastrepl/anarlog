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
vi.mock("~/shared/utils", () => {
  let counter = 0;
  return {
    DEFAULT_USER_ID: "default-user",
    id: () => `generated-${++counter}`,
  };
});

import type { IncomingParticipants } from "./fetch/types";
import { syncSessionParticipants } from "./process/participants/sync";
import { applyConnectionSync, loadParticipantSyncSnapshot } from "./storage";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");
let db: InstanceType<typeof DatabaseSync>;

const ctx = {
  provider: "google" as const,
  connectionId: "conn",
  from: new Date("2026-09-14"),
  to: new Date("2026-09-20"),
  calendarIds: new Set(["calendar"]),
  calendarTrackingIdToId: new Map([["primary", "calendar"]]),
};
const session = {
  id: "note-1",
  ownerUserId: "owner",
  eventJson: "{}",
  trackingId: "tracking-1",
};

async function sync(incoming: IncomingParticipants) {
  const snapshot = await loadParticipantSyncSnapshot([session], incoming);
  const participants = syncSessionParticipants({
    incomingParticipants: incoming,
    snapshot,
  });
  await applyConnectionSync({
    ctx,
    events: { toDelete: [], toUpdate: [], toAdd: [] },
    sessionUpdates: [],
    participants,
  });
}

function humans() {
  return db
    .prepare(
      `SELECT h.email, h.name, COALESCE(o.name, '') AS company
       FROM humans h LEFT JOIN organizations o ON o.id = h.organization_id
       WHERE h.deleted_at IS NULL ORDER BY h.email`,
    )
    .all() as Array<{ email: string; name: string; company: string }>;
}

describe("participant enrichment in SQLite", () => {
  beforeEach(() => {
    db = new DatabaseSync(":memory:");
    db.exec(`
      CREATE TABLE app_settings (id TEXT PRIMARY KEY, value_json TEXT, updated_at TEXT);
      CREATE TABLE sessions (
        id TEXT PRIMARY KEY, workspace_id TEXT, owner_user_id TEXT DEFAULT 'owner',
        created_at TEXT DEFAULT '2026-09-01', deleted_at TEXT
      );
      CREATE TABLE organizations (
        id TEXT PRIMARY KEY, workspace_id TEXT, owner_user_id TEXT, name TEXT, memo TEXT,
        pinned INTEGER, pin_order INTEGER, metadata_json TEXT, created_at TEXT, updated_at TEXT, deleted_at TEXT
      );
      CREATE TABLE humans (
        id TEXT PRIMARY KEY, workspace_id TEXT, owner_user_id TEXT, name TEXT DEFAULT '',
        email TEXT DEFAULT '', organization_id TEXT DEFAULT '', created_at TEXT, updated_at TEXT, deleted_at TEXT
      );
      CREATE TABLE session_participants (
        id TEXT PRIMARY KEY, workspace_id TEXT, owner_user_id TEXT, session_id TEXT, human_id TEXT,
        display_name TEXT, email TEXT, role TEXT, source TEXT, metadata_json TEXT,
        created_at TEXT, updated_at TEXT, deleted_at TEXT
      );
      INSERT INTO sessions(id) VALUES ('note-1');
      INSERT INTO humans(id, name, email, created_at, updated_at) VALUES ('owner', '', 'owner@acme.com', '2026-09-01', '2026-09-01');
    `);
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

  test("creates named humans linked to inferred organizations and converges", async () => {
    const incoming: IncomingParticipants = new Map([
      [
        "tracking-1",
        [
          { email: "simon.goldstein@ionprotocol.io" },
          { email: "jane.doe@gmail.com" },
          { email: "owner@acme.com" },
        ],
      ],
    ]);

    await sync(incoming);
    expect(humans()).toEqual([
      { email: "jane.doe@gmail.com", name: "Jane Doe", company: "" },
      { email: "owner@acme.com", name: "", company: "" },
      {
        email: "simon.goldstein@ionprotocol.io",
        name: "Simon Goldstein",
        company: "Ionprotocol",
      },
    ]);
    expect(
      db
        .prepare("SELECT display_name FROM session_participants ORDER BY email")
        .all(),
    ).toEqual([
      { display_name: "Jane Doe" },
      { display_name: "" },
      { display_name: "Simon Goldstein" },
    ]);

    await sync(incoming);
    expect(humans()).toHaveLength(3);
    expect(db.prepare("SELECT count(*) AS n FROM organizations").get()).toEqual(
      { n: 1 },
    );
  });

  test("enriches email-named humans, reuses organizations, and keeps user edits", async () => {
    db.exec(`
      INSERT INTO organizations(id, name, created_at, updated_at) VALUES ('org-acme', 'Acme', '2026-09-01', '2026-09-01');
      INSERT INTO humans(id, name, email, created_at, updated_at) VALUES
        ('h-alice', 'alice@acme.com', 'alice@acme.com', '2026-09-01', '2026-09-01'),
        ('h-bob', 'Robert Builder', 'bob@acme.com', '2026-09-01', '2026-09-01');
    `);
    const incoming: IncomingParticipants = new Map([
      [
        "tracking-1",
        [{ email: "alice@acme.com" }, { email: "bob@acme.com", name: "Bob" }],
      ],
    ]);

    await sync(incoming);
    expect(humans()).toEqual([
      { email: "alice@acme.com", name: "Alice", company: "Acme" },
      { email: "bob@acme.com", name: "Robert Builder", company: "Acme" },
      { email: "owner@acme.com", name: "", company: "" },
    ]);
    expect(db.prepare("SELECT count(*) AS n FROM organizations").get()).toEqual(
      { n: 1 },
    );
  });

  test("does not overwrite a name edited after the plan was computed", async () => {
    db.exec(
      "INSERT INTO humans(id, name, email, created_at, updated_at) VALUES ('h-alice', '', 'alice@acme.com', '2026-09-01', '2026-09-01')",
    );
    const incoming: IncomingParticipants = new Map([
      ["tracking-1", [{ email: "alice@acme.com" }]],
    ]);
    const snapshot = await loadParticipantSyncSnapshot([session], incoming);
    const participants = syncSessionParticipants({
      incomingParticipants: incoming,
      snapshot,
    });
    expect(participants.humansToEnrich[0]?.name).toBe("Alice");

    db.exec("UPDATE humans SET name = 'Alice Smith' WHERE id = 'h-alice'");
    await applyConnectionSync({
      ctx,
      events: { toDelete: [], toUpdate: [], toAdd: [] },
      sessionUpdates: [],
      participants,
    });
    expect(humans()[0]).toEqual({
      email: "alice@acme.com",
      name: "Alice Smith",
      company: "Acme",
    });
  });
});
