import { createRequire } from "node:module";
import type {
  DatabaseSync as SqliteDatabase,
  SQLInputValue,
} from "node:sqlite";
import { expect, test, vi } from "vitest";

const database = vi.hoisted(() => ({ current: null as SqliteDatabase | null }));
vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: async (sql: string, params: SQLInputValue[]) =>
      database.current!.prepare(sql).all(...params),
  },
  executeTransaction: async (
    statements: Array<{ sql: string; params: SQLInputValue[] }>,
  ) => {
    const db = database.current!;
    db.exec("BEGIN IMMEDIATE");
    try {
      const result = statements.map(({ sql, params }) =>
        db.prepare(sql).run(...params),
      );
      db.exec("COMMIT");
      return result;
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  },
}));

import {
  completeCaptureTranscript,
  loadCaptureStop,
  saveCaptureStop,
} from "./capture-completion";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");

test("completed captures keep their native stop clock through repair and atomically fill ad-hoc dates", async () => {
  const db = new DatabaseSync(":memory:");
  database.current = db;
  const start = Date.parse("2026-10-01T10:00:00Z");
  const stop = start + 60_000;
  try {
    db.exec(`
      CREATE TABLE app_settings (id TEXT PRIMARY KEY, value_json TEXT, updated_at TEXT);
      CREATE TABLE managed_app_settings (id TEXT PRIMARY KEY, value_json TEXT);
      INSERT INTO managed_app_settings VALUES ('intelligence_disabled', 'true');
      CREATE TABLE sessions (id TEXT PRIMARY KEY, started_at TEXT DEFAULT '',
        ended_at TEXT DEFAULT '', updated_at TEXT, deleted_at TEXT);
      CREATE TABLE transcripts (id TEXT PRIMARY KEY, session_id TEXT, started_at_ms INTEGER,
        ended_at_ms INTEGER, updated_at TEXT, deleted_at TEXT);
      INSERT INTO sessions (id) VALUES ('ad-hoc');
      INSERT INTO sessions (id, started_at, ended_at)
        VALUES ('calendar', '2026-10-01T09:55:00Z', '2026-10-01T10:30:00Z');
    `);
    const insert = db.prepare(
      "INSERT INTO transcripts (id, session_id, started_at_ms) VALUES (?, ?, ?)",
    );
    insert.run("live", "ad-hoc", start);
    insert.run("resumed", "ad-hoc", stop + 60_000);
    insert.run("repaired", "calendar", start);
    const usage = {
      startedAtMs: start,
      requestedLiveTranscription: true,
      liveTranscriptionActiveAtStop: false,
    };
    await saveCaptureStop("ad-hoc", "live", stop, usage);
    // Replayed recovery must retain the original end, even hours later.
    await saveCaptureStop("ad-hoc", "live", stop + 3_600_000, {
      ...usage,
      startedAtMs: stop,
    });
    expect(await loadCaptureStop("ad-hoc", "live")).toBe(stop);
    const savedUsage = db
      .prepare(
        "SELECT value_json FROM app_settings WHERE id='capture_usage:ad-hoc:live'",
      )
      .get()!;
    expect(JSON.parse(savedUsage.value_json as string)).toEqual({
      version: 1,
      sessionId: "ad-hoc",
      transcriptId: "live",
      stoppedAtMs: stop,
      ...usage,
    });
    await completeCaptureTranscript(
      "ad-hoc",
      "live",
      (await loadCaptureStop("ad-hoc", "live"))!,
    );
    expect(
      db
        .prepare("SELECT started_at, ended_at FROM sessions WHERE id='ad-hoc'")
        .get(),
    ).toEqual({
      started_at: "2026-10-01T10:00:00.000Z",
      ended_at: "2026-10-01T10:01:00.000Z",
    });
    expect(
      db
        .prepare("SELECT ended_at_ms FROM transcripts WHERE id='resumed'")
        .get(),
    ).toEqual({ ended_at_ms: null });
    await completeCaptureTranscript("calendar", "repaired", stop);
    expect(
      db
        .prepare(
          "SELECT started_at, ended_at FROM sessions WHERE id='calendar'",
        )
        .get(),
    ).toEqual({
      started_at: "2026-10-01T09:55:00Z",
      ended_at: "2026-10-01T10:01:00.000Z",
    });
    db.exec(
      "CREATE TRIGGER fail_completion BEFORE UPDATE ON sessions WHEN NEW.id='ad-hoc' BEGIN SELECT RAISE(ABORT, 'write failed'); END",
    );
    await expect(
      completeCaptureTranscript("ad-hoc", "resumed", stop + 120_000),
    ).rejects.toThrow("write failed");
    expect(
      db
        .prepare("SELECT ended_at_ms FROM transcripts WHERE id='resumed'")
        .get(),
    ).toEqual({ ended_at_ms: null });
  } finally {
    database.current = null;
    db.close();
  }
});
