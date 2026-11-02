import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { registerHooks } from "node:module";
import { test } from "node:test";

const fixture = (globalThis.meetingGlossaryFixture = {
  db: new DatabaseSync(":memory:"),
});
fixture.db.exec(`CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL DEFAULT '',
  deleted_at TEXT
)`);
registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "@/db")
      return { url: "test:meeting-glossary-db", shortCircuit: true };
    if (specifier === "@/lib/ids")
      return { url: "test:meeting-glossary-ids", shortCircuit: true };
    return next(specifier, context);
  },
  load(url, context, next) {
    if (url === "test:meeting-glossary-db")
      return {
        format: "module",
        shortCircuit: true,
        source: `
          const fixture = globalThis.meetingGlossaryFixture;
          export async function execute(sql, params = []) {
            return fixture.db.prepare(sql).all(...params);
          }
          export async function executeTransaction(statements) {
            fixture.db.exec("BEGIN");
            try {
              for (const statement of statements)
                fixture.db.prepare(statement.sql).run(...(statement.params ?? []));
              fixture.db.exec("COMMIT");
            } catch (error) {
              fixture.db.exec("ROLLBACK");
              throw error;
            }
          }
        `,
      };
    if (url === "test:meeting-glossary-ids")
      return {
        format: "module",
        shortCircuit: true,
        source: `export const nowIso = () => "2026-11-03T00:00:00.000Z";`,
      };
    return next(url, context);
  },
});

const {
  mergeTranscriptionKeywords,
  normalizeMeetingGlossary,
  parseMeetingGlossaryText,
  readMeetingGlossary,
  saveMeetingGlossary,
} = await import("./meeting-glossary.ts");

test("meeting glossary trims, deduplicates, and validates user terms", () => {
  assert.deepEqual(
    parseMeetingGlossaryText(" Cloudflare  \nPostgreSQL\ncloudflare\n \n"),
    ["Cloudflare", "PostgreSQL"],
  );
  assert.throws(() => normalizeMeetingGlossary(["x".repeat(101)]), /100 characters/u);
  assert.throws(
    () => normalizeMeetingGlossary(["😀".repeat(101)]),
    /100 characters/u,
  );
  assert.throws(
    () =>
      normalizeMeetingGlossary(
        Array.from({ length: 11 }, (_, index) => "😀".repeat(98) + index),
      ),
    /4,000 characters/u,
  );
  assert.throws(
    () => normalizeMeetingGlossary(Array.from({ length: 101 }, (_, i) => `term-${i}`)),
    /up to 100 terms/u,
  );
});

test("meeting terms take precedence over the shared dictionary within decoder limits", () => {
  assert.deepEqual(
    mergeTranscriptionKeywords(
      ["Project X", "Cloudflare"],
      ["cloudflare", "PostgreSQL"],
    ),
    ["Project X", "Cloudflare", "PostgreSQL"],
  );
  assert.equal(
    mergeTranscriptionKeywords(
      ["meeting"],
      Array.from({ length: 100 }, (_, index) => `shared-${index}`),
    ).length,
    100,
  );
});

test("meeting glossary is persisted in session metadata without replacing other fields", async () => {
  fixture.db
    .prepare("INSERT INTO sessions (id, metadata_json) VALUES (?, ?)")
    .run("meeting-1", JSON.stringify({ custom: "keep" }));

  await saveMeetingGlossary("meeting-1", [" Cloudflare ", "PostgreSQL"]);
  assert.deepEqual(await readMeetingGlossary("meeting-1"), [
    "Cloudflare",
    "PostgreSQL",
  ]);
  const row = fixture.db
    .prepare("SELECT metadata_json, updated_at FROM sessions WHERE id = ?")
    .get("meeting-1");
  assert.deepEqual(JSON.parse(row.metadata_json), {
    custom: "keep",
    transcription_glossary: ["Cloudflare", "PostgreSQL"],
  });
  assert.equal(row.updated_at, "2026-11-03T00:00:00.000Z");
});

test("glossary recovery ignores invalid metadata and can replace it safely", async () => {
  fixture.db
    .prepare("INSERT INTO sessions (id, metadata_json) VALUES (?, ?)")
    .run("meeting-2", "{broken");
  assert.deepEqual(await readMeetingGlossary("meeting-2"), []);
  await saveMeetingGlossary("meeting-2", ["staging"]);
  assert.deepEqual(await readMeetingGlossary("meeting-2"), ["staging"]);
});
