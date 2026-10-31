import { QueryClient } from "@tanstack/react-query";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { setTimeout } from "node:timers/promises";

const root = new URL("../", import.meta.url);
const fixture = (globalThis.transcriptionRetryFixture = {
  sessionId: "live-only-recording",
  provider: {
    provider: "dashscope",
    model: "qwen3-asr-flash-realtime",
    apiKey: "synthetic-key",
  },
  pending: true,
  captureStartedAt: 1730000000000,
  resolutions: 0,
  requests: 0,
  transactions: [],
  failures: [],
  summaries: [],
  requestError: null,
});
const mocks = {
  "./summarize": `export function generateSummaryAfterTranscription(sessionId) { globalThis.transcriptionRetryFixture.summaries.push(sessionId); }`,
  "expo-file-system": `export const Paths = {document: '/synthetic'}; export class File { constructor(...parts) {this.uri = parts.join('/');} exists = true; size = 1024; }`,
  "expo/fetch": `export function fetch() { throw new Error('Unexpected hosted request'); }`,
  "react-native": `export const Platform = {OS: 'ios'}; export const AppState = {currentState: 'active', addEventListener: () => ({remove() {}})};`,
  "@/auth/billing": `export class ProRequiredError extends Error {}`,
  "@/db": `const f = globalThis.transcriptionRetryFixture;
    export async function execute(sql, params) {
      if (!f.pending) return [];
      if (sql.includes('SELECT attachment.session_id')) return f.db ? f.db.prepare(sql).all(...params) : [{session_id: f.sessionId}];
      return [{content_type: 'audio/wav', size_bytes: 1024, local_relative_path: 'audio.wav', capture_started_at_ms: f.captureStartedAt}];
    }
    export async function executeTransaction(statements) {
      f.transactions.push(statements);
      if (f.db) {
        const completed = statements.find(s => s.sql.includes('UPDATE session_attachments'));
        f.db.prepare("UPDATE session_attachments SET metadata_json = ? WHERE session_id = ?").run(JSON.stringify({transcript_status: 'complete'}), completed.params[2]);
      } else f.pending = false;
    }
  `,
  "@/settings/providers": `export async function readProviderConfig() { return globalThis.transcriptionRetryFixture.provider; } export async function resolveProvider() { const f = globalThis.transcriptionRetryFixture; f.resolutions++; return f.provider; }`,
  "@/settings/preferences": `export async function readPreferences() { return {ai_language: 'en', spoken_languages: [], personalization_dictionary_terms: []}; }`,
  "@/lib/ids": `import {randomUUID} from 'node:crypto'; export const id = randomUUID; export const nowIso = () => new Date().toISOString();`,
  "@/lib/env": `export const env = {apiUrl: 'https://api.anarlog.test'};`,
  "@/lib/analytics": `export function captureAnalytics() {}`,
  "@/lib/error-reporting": `export function captureOperationalError(error) { globalThis.transcriptionRetryFixture.failures.push(error); }`,
  "./local-speech": `export async function transcribeLocalFile(uri) {
    if (globalThis.transcriptionRetryFixture.modelMissing) throw Object.assign(new Error('Download the speech model'), {code: 'ERR_MLKIT_MODEL_REQUIRED'});
    if (uri.includes('temporary-failure')) throw new Error('Temporary failure');
    if (uri.includes('unsupported-')) throw Object.assign(new Error('Unsupported audio'), {code: 'ERR_MLKIT_AUDIO_FORMAT'});
    return 'Recovered recording';
  }`,
  "./provider-transcription": `export async function requestProviderTranscription() {
    const f = globalThis.transcriptionRetryFixture; f.requests++;
    if (f.requestError) throw f.requestError;
    return {status: 200, body: JSON.stringify({text: 'Recovered recording'})};
  }`,
};
registerHooks({
  resolve(specifier, context, next) {
    if (mocks[specifier])
      return {
        url: `data:text/javascript,${encodeURIComponent(mocks[specifier])}`,
        shortCircuit: true,
      };
    if (specifier.startsWith("@/"))
      return {
        url: new URL(`${specifier.slice(2)}.ts`, root).href,
        shortCircuit: true,
      };
    if (specifier.startsWith(".") && context.parentURL?.startsWith(root.href)) {
      const url = new URL(`${specifier}.ts`, context.parentURL);
      if (existsSync(url)) return { url: url.href, shortCircuit: true };
    }
    return next(specifier, context);
  },
});

const {
  retryPendingTranscriptions,
  retryLocalTranscriptions,
  transcribeSession,
} = await import("./transcribe.ts");

test("live-only recordings stop automatic retries but can be recovered manually with another provider", async () => {
  await transcribeSession(fixture.sessionId);
  assert.equal(fixture.failures.at(-1).code, "stt_live_only");
  assert.equal(fixture.pending, true);
  assert.equal(fixture.requests, 0);
  assert.deepEqual(fixture.summaries, []);
  assert.equal(fixture.resolutions, 1);

  await retryPendingTranscriptions();
  await retryPendingTranscriptions();
  assert.equal(fixture.resolutions, 1);
  assert.equal(fixture.failures.length, 1);

  fixture.provider = {
    provider: "openai",
    model: "whisper-1",
    apiKey: "synthetic-key",
  };
  await transcribeSession(fixture.sessionId);
  assert.equal(fixture.requests, 1);
  assert.equal(fixture.transactions.length, 1);
  assert.equal(fixture.pending, false);
  const persistedTranscript = fixture.transactions
    .at(-1)
    .find(({ sql }) => sql.includes("INSERT INTO transcripts"));
  assert.equal(persistedTranscript.params[4], fixture.captureStartedAt);
  assert.ok(fixture.summaries.includes(fixture.sessionId));
});

test("temporary request failures remain eligible for automatic retry", async () => {
  fixture.sessionId = "transient-recording";
  fixture.provider = {
    provider: "openai",
    model: "whisper-1",
    apiKey: "synthetic-key",
  };
  fixture.pending = true;
  fixture.requestError = new Error("Temporary connection failure");
  const requests = fixture.requests;
  await transcribeSession(fixture.sessionId);
  assert.equal(fixture.pending, true);
  assert.equal(fixture.requests, requests + 1);

  fixture.requestError = null;
  await retryPendingTranscriptions();
  assert.equal(fixture.requests, requests + 2);
  assert.equal(fixture.pending, false);
  assert.ok(fixture.summaries.includes(fixture.sessionId));
});

test("eight unsupported local imports do not starve a later recording's automatic recovery", async () => {
  fixture.db = new DatabaseSync(":memory:");
  fixture.db.exec(`
    CREATE TABLE session_attachments (
      id TEXT, session_id TEXT, source_type TEXT, deleted_at TEXT,
      size_bytes INTEGER, metadata_json TEXT, updated_at INTEGER
    );
    CREATE TABLE attachment_local_state (attachment_id TEXT, availability TEXT);
  `);
  fixture.provider = { provider: "google_on_device", model: "mlkit-advanced" };
  fixture.pending = true;
  try {
    for (let i = 0; i < 9; i++) {
      const sessionId = i < 8 ? `unsupported-${i}` : "supported-recording";
      fixture.db
        .prepare(
          "INSERT INTO session_attachments VALUES (?, ?, 'session_audio', NULL, 1024, '{}', ?)",
        )
        .run(sessionId, sessionId, i);
      fixture.db
        .prepare("INSERT INTO attachment_local_state VALUES (?, 'present')")
        .run(sessionId);
      if (i < 8) await transcribeSession(sessionId);
    }
    assert.equal(fixture.failures.at(-1).code, "ERR_MLKIT_AUDIO_FORMAT");
    await retryPendingTranscriptions();
    assert.ok(fixture.summaries.includes("supported-recording"));
  } finally {
    fixture.db.close();
    fixture.db = null;
  }
});

function seedLocalRecovery(recordings) {
  fixture.db = new DatabaseSync(":memory:");
  fixture.db.exec(`
    CREATE TABLE app_settings (id TEXT, value_json TEXT);
    CREATE TABLE sessions (id TEXT, owner_user_id TEXT, deleted_at TEXT);
    CREATE TABLE local_library_connections (account_user_id TEXT, library_workspace_id TEXT, active INTEGER);
    CREATE TABLE session_attachments (
      id TEXT, session_id TEXT, source_type TEXT, deleted_at TEXT,
      size_bytes INTEGER, metadata_json TEXT, updated_at INTEGER
    );
    CREATE TABLE attachment_local_state (attachment_id TEXT, availability TEXT);
    INSERT INTO local_library_connections VALUES ('account-a', 'library-a', 1);
  `);
  fixture.provider = { provider: "google_on_device", model: "mlkit-advanced" };
  fixture.pending = true;
  recordings.forEach(([id, owner], index) => {
    fixture.db
      .prepare("INSERT INTO sessions VALUES (?, ?, NULL)")
      .run(id, owner);
    fixture.db
      .prepare(
        "INSERT INTO session_attachments VALUES (?, ?, 'session_audio', NULL, 1024, '{}', ?)",
      )
      .run(id, id, index);
    fixture.db
      .prepare("INSERT INTO attachment_local_state VALUES (?, 'present')")
      .run(id);
  });
}

function completedRecordings() {
  return fixture.db
    .prepare(
      "SELECT session_id FROM session_attachments WHERE json_extract(metadata_json, '$.transcript_status') = 'complete' ORDER BY session_id",
    )
    .all()
    .map((row) => row.session_id);
}

test(
  "local recovery drains more than eight recordings without looping on a transient failure",
  { timeout: 3000 },
  async () => {
    const recordings = Array.from({ length: 12 }, (_, i) => [
      `drain-${i}`,
      "account-a",
    ]);
    seedLocalRecovery([["temporary-failure", "account-a"], ...recordings]);
    try {
      await retryLocalTranscriptions("account-a", async () => true);
      assert.deepEqual(
        completedRecordings(),
        recordings.map(([id]) => id).sort(),
      );
    } finally {
      fixture.db.close();
      fixture.db = null;
    }
  },
);

test("local recovery only processes the current account and its local library", async () => {
  seedLocalRecovery([
    ["current-account", "account-a"],
    ["current-library", "library-a"],
    ["other-account", "account-b"],
    ["other-library", "library-b"],
    ["unowned", null],
  ]);
  try {
    await retryLocalTranscriptions("account-a", async () => true);
    assert.deepEqual(completedRecordings(), [
      "current-account",
      "current-library",
      "unowned",
    ]);
    await retryLocalTranscriptions(null, async () => true);
    assert.deepEqual(completedRecordings(), [
      "current-account",
      "current-library",
      "unowned",
    ]);
  } finally {
    fixture.db.close();
    fixture.db = null;
  }
});

test("local recovery stops scheduling recordings when its lifecycle becomes inactive", async () => {
  seedLocalRecovery([
    ["stop-first", "account-a"],
    ["stop-second", "account-a"],
  ]);
  try {
    await retryLocalTranscriptions(
      "account-a",
      async () => completedRecordings().length === 0,
    );
    assert.deepEqual(completedRecordings(), ["stop-first"]);
  } finally {
    fixture.db.close();
    fixture.db = null;
  }
});

test("local recovery includes an unconnected device workspace without crossing account boundaries", async () => {
  for (const { account, boundAccount, connected, expected } of [
    { account: null, boundAccount: null, connected: false, expected: true },
    {
      account: "account-a",
      boundAccount: null,
      connected: false,
      expected: true,
    },
    {
      account: "account-a",
      boundAccount: "account-a",
      connected: false,
      expected: true,
    },
    {
      account: "account-a",
      boundAccount: "account-b",
      connected: false,
      expected: false,
    },
    { account: null, boundAccount: null, connected: true, expected: false },
  ]) {
    seedLocalRecovery([
      ["local-workspace-recording", "device-workspace"],
      ["foreign-workspace-recording", "foreign-workspace"],
    ]);
    try {
      fixture.db.exec("DELETE FROM local_library_connections");
      fixture.db
        .prepare(
          "INSERT INTO app_settings VALUES ('cloudsync_workspace_binding', ?)",
        )
        .run(
          JSON.stringify({
            workspace_id: "device-workspace",
            account_user_id: boundAccount,
          }),
        );
      if (connected) {
        fixture.db.exec(
          "INSERT INTO local_library_connections VALUES ('account-b', 'device-workspace', 0)",
        );
      }
      await retryLocalTranscriptions(account, async () => true);
      assert.deepEqual(
        completedRecordings(),
        expected ? ["local-workspace-recording"] : [],
      );
    } finally {
      fixture.db.close();
      fixture.db = null;
    }
  }
});

test("downloading the active engine's model recovers pending recordings without another lifecycle event", async () => {
  const { startLocalTranscriptionRecovery } =
    await import("./local-transcription-recovery.ts");
  const queryClient = new QueryClient();
  seedLocalRecovery([["awaiting-model", "account-a"]]);
  fixture.modelMissing = true;
  const stop = startLocalTranscriptionRecovery("account-a", queryClient);
  try {
    for (
      let i = 0;
      i < 100 && fixture.failures.at(-1)?.code !== "ERR_MLKIT_MODEL_REQUIRED";
      i++
    ) {
      await setTimeout(1);
    }
    assert.equal(fixture.failures.at(-1)?.code, "ERR_MLKIT_MODEL_REQUIRED");
    assert.deepEqual(completedRecordings(), []);
    const failuresBeforeReady = fixture.failures.length;
    for (const [language, model] of [
      ["en", "mlkit-basic"],
      ["fr", "mlkit-advanced"],
    ]) {
      await queryClient.fetchQuery({
        queryKey: ["local-speech-model", language, model],
        queryFn: async () => ({ status: "available", language }),
      });
      await setTimeout(10);
      assert.equal(fixture.failures.length, failuresBeforeReady);
      assert.deepEqual(completedRecordings(), []);
    }
    fixture.modelMissing = false;
    await queryClient.fetchQuery({
      queryKey: ["local-speech-model", "en", "mlkit-advanced"],
      queryFn: async () => ({ status: "available", language: "en" }),
    });
    for (let i = 0; i < 100 && completedRecordings().length === 0; i++) {
      await setTimeout(1);
    }
    assert.deepEqual(completedRecordings(), ["awaiting-model"]);
  } finally {
    stop();
    queryClient.clear();
    fixture.modelMissing = false;
    fixture.db.close();
    fixture.db = null;
  }
});
