import { createRequire } from "node:module";
import { expect, it, vi } from "vitest";

const { executeTransaction, useLiveQuery } = vi.hoisted(() => ({
  executeTransaction: vi.fn(),
  useLiveQuery: vi.fn(),
}));
vi.mock("~/db", () => ({ executeTransaction, useLiveQuery }));
vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  saveIncompleteCapture,
  clearIncompleteCapture,
  clearCaptureAudioDeletionFailure,
  useIncompleteCapture,
} from "./capture-result";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");

it("preserves failed audio deletion across recovery until cleanup is explicitly confirmed", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec(
    "CREATE TABLE app_settings (id TEXT PRIMARY KEY, value_json TEXT, updated_at TEXT)",
  );
  executeTransaction.mockImplementation(
    async (statements: Array<{ sql: string; params: string[] }>) => {
      for (const statement of statements)
        db.prepare(statement.sql).run(...statement.params);
    },
  );
  const read = () =>
    JSON.parse(
      db.prepare("SELECT value_json FROM app_settings").get()!
        .value_json as string,
    );
  try {
    await saveIncompleteCapture("session", "transcript", false, true);
    await saveIncompleteCapture("session", "transcript", false);
    expect(read()).toEqual({ audioDeleted: false, audioDeletionFailed: true });
    await clearIncompleteCapture("session", "transcript");
    expect(read().audioDeletionFailed).toBe(true);
    await saveIncompleteCapture("session", "audio-cleanup", false, true);
    await clearCaptureAudioDeletionFailure("session");
    expect(db.prepare("SELECT count(*) AS n FROM app_settings").get()!.n).toBe(
      1,
    );
    expect(read()).toEqual({ audioDeleted: true, audioDeletionFailed: false });
  } finally {
    db.close();
    executeTransaction.mockReset();
  }
});

it("shows warnings for visible transcript captures and preserves deletion failures independently", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(`CREATE TABLE app_settings (id TEXT PRIMARY KEY, value_json TEXT);
    CREATE TABLE transcripts (id TEXT PRIMARY KEY, session_id TEXT, deleted_at TEXT);
    INSERT INTO transcripts VALUES ('old', 'session', 'deleted'), ('current', 'session', NULL);
    INSERT INTO app_settings VALUES ('capture_incomplete:session:old', '{"audioDeleted":true}');`);
  useLiveQuery.mockImplementation(({ sql, params, mapRows }) => ({
    data: mapRows(db.prepare(sql).all(...params)),
  }));
  try {
    expect(useIncompleteCapture("session")).toBeNull();
    db.exec(
      `INSERT INTO app_settings VALUES ('capture_incomplete:session:current', '{"audioDeleted":false}');`,
    );
    expect(useIncompleteCapture("session")).toEqual({
      audioDeleted: false,
      audioDeletionFailed: false,
    });
    db.exec(
      `UPDATE app_settings SET value_json = '{"audioDeletionFailed":true}' WHERE id = 'capture_incomplete:session:old';`,
    );
    expect(useIncompleteCapture("session")?.audioDeletionFailed).toBe(true);
  } finally {
    db.close();
    useLiveQuery.mockReset();
  }
});
