import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { after, test } from "node:test";

const db = new DatabaseSync(":memory:");
globalThis.audioCatalogPersistence = db;
const mocks = {
  "@/db": `export function useLiveQuery() {} export async function execute(sql, params) { return globalThis.audioCatalogPersistence.prepare(sql).all(...params); } export async function executeTransaction(statements) {
    const db = globalThis.audioCatalogPersistence; db.exec('BEGIN');
    try { const result = statements.map(({sql, params}) => db.prepare(sql).run(...params).changes); db.exec('COMMIT'); return result; }
    catch (error) { db.exec('ROLLBACK'); throw error; }
  }`,
  "expo-file-system": `export const Paths = {document: '/fixture'}; export const FileMode = {ReadOnly: 0, ReadWrite: 1}; export class File {
    constructor(...parts) { this.path = parts.join('/'); }
    get exists() {return this.path === globalThis.recoveryFilePath;}
    delete() {globalThis.recoveryFilePath = null;}
    get size() {return 3244;}
    open() {return {offset: 0, readBytes() {return globalThis.recoveryHeader;}, writeBytes(bytes) {globalThis.recoveryHeader = bytes;}, close() {}};}
  }`,
  "./file-sha256": `export async function hashFileSha256() { return {sha256: 'a'.repeat(64), sizeBytes: 44}; }`,
  "@/attachment-sync/upload-runner": `export function requestMobileAttachmentUploads() {}`,
  "@/attachment-sync/upload-store": `export function requeueMobileAttachmentUpload() {}`,
  "@/lib/ids": `export {randomUUID as id} from 'node:crypto'; export const nowIso = () => new Date().toISOString();`,
  "@/data/transcribe": `export async function transcribeSession() {}`,
  "@/lib/analytics": `export function captureAnalytics() {}`,
  "@/lib/error-reporting": `export function captureOperationalError(error) {throw error;}`,
  "@/live-activity/meeting-recording-activity": `export async function clearStaleMeetingRecordingActivities() {}`,
};
const hooks = registerHooks({
  resolve(specifier, context, next) {
    if (mocks[specifier])
      return {
        url: `data:text/javascript,${encodeURIComponent(mocks[specifier])}`,
        shortCircuit: true,
      };
    if (specifier === "./audio-catalog-model")
      return {
        url: new URL("./audio-catalog-model.ts", import.meta.url).href,
        shortCircuit: true,
      };
    if (specifier === "@/data/audio-catalog")
      return {
        url: new URL("./audio-catalog.ts", import.meta.url).href,
        shortCircuit: true,
      };
    if (specifier === "@/audio/pcm-wav")
      return {
        url: new URL("../audio/pcm-wav.ts", import.meta.url).href,
        shortCircuit: true,
      };
    if (specifier === "@/audio/capture-lifecycle")
      return {
        url: new URL("../audio/capture-lifecycle.ts", import.meta.url).href,
        shortCircuit: true,
      };
    return next(specifier, context);
  },
});
const { recordSessionCaptureStart, catalogSessionAudio } =
  await import("./audio-catalog.ts");
for (const name of [
  "20260710223922_canonical_data_model.sql",
  "20260717140000_attachment_local_state.sql",
  "20260717150000_attachment_transfer_jobs.sql",
  "20260717170000_attachment_cloud_sync_intent.sql",
]) {
  db.exec(
    readFileSync(
      new URL(`../../../../crates/db-app/migrations/${name}`, import.meta.url),
      "utf8",
    ),
  );
}
after(() => {
  hooks.deregister();
  db.close();
  delete globalThis.audioCatalogPersistence;
});

test("recording recovery preserves the durable capture time while imports ignore stale recording metadata", async () => {
  db.exec("INSERT INTO sessions (id) VALUES ('recorded'), ('imported')");
  for (const sessionId of ["recorded", "imported"])
    await recordSessionCaptureStart(sessionId);
  const capture = db
    .prepare(
      "SELECT json_extract(metadata_json, '$.mobile_capture_started_at_ms') AS time FROM sessions WHERE id = 'recorded'",
    )
    .get().time;
  assert.ok(Number.isFinite(capture));
  await catalogSessionAudio("recorded", {
    source: "recording",
    filename: "audio.wav",
    contentType: "audio/wav",
  });
  await catalogSessionAudio("imported", {
    source: "import",
    filename: "audio.wav",
    contentType: "audio/wav",
  });
  const metadata = (sessionId) =>
    JSON.parse(
      db
        .prepare(
          "SELECT metadata_json FROM session_attachments WHERE session_id = ?",
        )
        .get(sessionId).metadata_json,
    );
  assert.equal(metadata("recorded").capture_started_at_ms, capture);
  assert.equal(metadata("imported").capture_started_at_ms, null);
  // Recovery re-catalogs from durable session state without starting a new capture.
  await catalogSessionAudio("recorded", {
    source: "recording",
    filename: "audio.wav",
    contentType: "audio/wav",
  });
  assert.equal(metadata("recorded").capture_started_at_ms, capture);
});

test("resumed audio publication keeps the original timeline and clears its recovery marker atomically", async () => {
  db.exec("INSERT INTO sessions (id) VALUES ('resumed')");
  await recordSessionCaptureStart("resumed", {
    filename: "audio-resume-abc.wav",
    startedAtMs: 123456,
  });
  const before = JSON.parse(
    db
      .prepare(
        "SELECT value_json FROM app_settings WHERE id = 'mobile_capture_pending:resumed'",
      )
      .get().value_json,
  );
  assert.equal(before.filename, "audio-resume-abc.wav");
  await catalogSessionAudio("resumed", {
    source: "recording",
    filename: "audio-resume-abc.wav",
    contentType: "audio/wav",
  });
  const audio = db
    .prepare(
      "SELECT relative_path, metadata_json FROM session_attachments WHERE session_id = 'resumed'",
    )
    .get();
  assert.equal(audio.relative_path, "audio-resume-abc.wav");
  assert.equal(JSON.parse(audio.metadata_json).capture_started_at_ms, 123456);
  assert.equal(
    db
      .prepare(
        "SELECT value_json FROM app_settings WHERE id = 'mobile_capture_pending:resumed'",
      )
      .get(),
    undefined,
  );
});

test("startup recovers appended audio even when the session already has a saved recording", async () => {
  const { wavHeader } = await import("../audio/pcm-wav.ts");
  const { recoverInterruptedRecordings } =
    await import("../audio/recover-recordings.ts");
  db.exec("INSERT INTO sessions (id) VALUES ('interrupted')");
  await catalogSessionAudio("interrupted", {
    source: "recording",
    filename: "audio.wav",
    contentType: "audio/wav",
  });
  db.exec(
    "UPDATE session_attachments SET cloud_sync_enabled = 0, metadata_json = json_set(metadata_json, '$.custom', 'preserved') WHERE session_id = 'interrupted'",
  );
  await recordSessionCaptureStart("interrupted", {
    filename: "audio-resume-abc.wav",
    startedAtMs: 123456,
    previousSizeBytes: 1644,
  });
  globalThis.recoveryFilePath =
    "/fixture/sessions/interrupted/audio-resume-abc.wav";
  globalThis.recoveryHeader = wavHeader(1600, 16000, 1);
  assert.equal(await recoverInterruptedRecordings(null), 1);
  const saved = db
    .prepare(
      "SELECT relative_path, metadata_json, cloud_sync_enabled FROM session_attachments WHERE session_id = 'interrupted'",
    )
    .get();
  assert.equal(saved.relative_path, "audio-resume-abc.wav");
  assert.equal(JSON.parse(saved.metadata_json).capture_started_at_ms, 123456);
  assert.equal(JSON.parse(saved.metadata_json).custom, "preserved");
  assert.equal(saved.cloud_sync_enabled, 0);
  assert.equal(
    db
      .prepare(
        "SELECT value_json FROM app_settings WHERE id = 'mobile_capture_pending:interrupted'",
      )
      .get(),
    undefined,
  );
  assert.equal(
    new DataView(globalThis.recoveryHeader.buffer).getUint32(40, true),
    3200,
  );
});

test("a crash before resumed PCM arrives preserves the original transcript and catalog", async () => {
  const { recoverInterruptedRecordings } =
    await import("../audio/recover-recordings.ts");
  db.exec(
    "INSERT INTO sessions (id) VALUES ('empty-resume'); INSERT INTO transcripts (id, session_id, words_json) VALUES ('edited', 'empty-resume', '[{\"text\":\"Corrected\"}]');",
  );
  await catalogSessionAudio("empty-resume", {
    source: "recording",
    filename: "audio.wav",
    contentType: "audio/wav",
  });
  db.exec(
    "UPDATE session_attachments SET metadata_json = json_set(metadata_json, '$.transcript_status', 'complete') WHERE session_id = 'empty-resume'",
  );
  await recordSessionCaptureStart("empty-resume", {
    filename: "audio-resume-abc.wav",
    startedAtMs: 123456,
    previousSizeBytes: 3244,
  });
  globalThis.recoveryFilePath =
    "/fixture/sessions/empty-resume/audio-resume-abc.wav";
  assert.equal(await recoverInterruptedRecordings(null), 0);
  const audio = db
    .prepare(
      "SELECT relative_path, metadata_json FROM session_attachments WHERE session_id = 'empty-resume'",
    )
    .get();
  assert.equal(audio.relative_path, "audio.wav");
  assert.equal(JSON.parse(audio.metadata_json).transcript_status, "complete");
  assert.equal(
    db
      .prepare(
        "SELECT words_json FROM transcripts WHERE id = 'edited' AND deleted_at IS NULL",
      )
      .get().words_json,
    '[{"text":"Corrected"}]',
  );
  assert.equal(
    db
      .prepare(
        "SELECT value_json FROM app_settings WHERE id = 'mobile_capture_pending:empty-resume'",
      )
      .get(),
    undefined,
  );
  assert.equal(globalThis.recoveryFilePath, null);
});
