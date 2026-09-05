import { createRequire } from "node:module";
import { describe, expect, it, vi } from "vitest";

vi.mock("~/auth", () => ({ useAuth: vi.fn() }));
vi.mock("~/db", () => ({ useLiveQuery: vi.fn() }));

import { ACTIVITY_SQL } from "./queries";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");

describe("personal activity query", () => {
  it("excludes other owners, deleted data, blank transcripts, and malformed JSON", () => {
    const db = new DatabaseSync(":memory:");
    try {
      db.exec(`
        CREATE TABLE sessions (id TEXT PRIMARY KEY, owner_user_id TEXT, deleted_at TEXT);
        CREATE TABLE transcripts (id TEXT PRIMARY KEY, session_id TEXT, started_at_ms INTEGER, created_at TEXT, words_json TEXT, deleted_at TEXT);
        INSERT INTO sessions VALUES ('mine', 'user', NULL), ('theirs', 'another-user', NULL), ('deleted', 'user', '2026-01-01'), ('guest', NULL, NULL);
      `);
      const insert = db.prepare(
        "INSERT INTO transcripts VALUES (?, ?, 1000, '2026-09-04T00:00:00Z', ?, ?)",
      );
      const words = JSON.stringify([
        { text: "Hello", end_ms: 400 },
        { text: "world", end_ms: 800 },
      ]);
      insert.run("mine", "mine", words, null);
      insert.run("theirs", "theirs", words, null);
      insert.run("deleted-session", "deleted", words, null);
      insert.run("deleted-transcript", "mine", words, "2026-01-01");
      insert.run("guest", "guest", words, null);
      for (const [index, json] of [
        "invalid",
        "[]",
        "{}",
        '["invalid"]',
        '[{"text":"  "}]',
      ].entries()) {
        insert.run(String(index), "mine", json, null);
      }
      const rows = db.prepare(ACTIVITY_SQL).all("user");
      expect(rows).toHaveLength(2);
      expect(rows).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ session_id: "mine", duration_ms: 800 }),
          expect.objectContaining({ session_id: "guest", duration_ms: 800 }),
        ]),
      );
      expect(db.prepare(ACTIVITY_SQL).all("guest-user")).toEqual([
        expect.objectContaining({ session_id: "guest", duration_ms: 800 }),
      ]);
    } finally {
      db.close();
    }
  });
});
