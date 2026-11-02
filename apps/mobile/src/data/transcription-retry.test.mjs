import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import { test } from "node:test";

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
  localCalls: [],
  checkpointsDeleted: [],
  localError: null,
  transactionError: null,
  requestError: null,
});
const mocks = {
  "./summarize": `export function generateSummaryAfterTranscription(sessionId) { globalThis.transcriptionRetryFixture.summaries.push(sessionId); }`,
  "./local-whisper": `export async function transcribeLocally(uri, params) { const f = globalThis.transcriptionRetryFixture; f.localCalls.push({uri, params}); if (f.localError) throw f.localError; return [{text: 'Chào team.', start_ms: 0, end_ms: 500, speaker: null, confidence: null}]; } export function discardLocalWhisperCheckpoint(uri) { globalThis.transcriptionRetryFixture.checkpointsDeleted.push(uri); }`,
  "expo-file-system": `export const Paths = {document: '/synthetic'}; export class Directory {} export class File { exists = true; size = 1024; }`,
  "expo/fetch": `export function fetch() { throw new Error('Unexpected hosted request'); }`,
  "react-native": `export const Platform = {OS: 'ios'};`,
  "@anlg/mobile-bridge": `export function localWhisperModelInfo() { throw new Error('Unexpected native model lookup'); } export async function transcribeLocalWhisper() { throw new Error('Unexpected local transcription'); } export async function verifyLocalWhisperModel() { return false; }`,
  "@/auth/billing": `export class ProRequiredError extends Error {}`,
  "@/db": `const f = globalThis.transcriptionRetryFixture;
    export async function execute(sql, params) {
      if (!f.pending) return [];
      if (sql.includes('SELECT attachment.session_id')) return [{session_id: f.sessionId}];
      return [{content_type: 'audio/wav', size_bytes: 1024, local_relative_path: 'audio.wav', capture_started_at_ms: f.captureStartedAt}];
    }
    export async function executeTransaction(statements) { if (f.transactionError) throw f.transactionError; f.transactions.push(statements); f.pending = false; }
  `,
  "@/settings/providers": `export async function resolveProvider() { const f = globalThis.transcriptionRetryFixture; f.resolutions++; return f.provider; }`,
  "@/settings/preferences": `export async function readPreferences() { return globalThis.transcriptionRetryFixture.preferences ?? {ai_language: 'en', spoken_languages: ['vi'], personalization_dictionary_terms: ['Cloudflare']}; }`,
  "@/lib/ids": `import {randomUUID} from 'node:crypto'; export const id = randomUUID; export const nowIso = () => new Date().toISOString();`,
  "@/lib/env": `export const env = {apiUrl: 'https://api.anarlog.test'};`,
  "@/lib/analytics": `export function captureAnalytics() {}`,
  "@/lib/error-reporting": `export function captureOperationalError(error) { globalThis.transcriptionRetryFixture.failures.push(error); }`,
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

const { retryPendingTranscriptions, transcribeSession } =
  await import("./transcribe.ts");

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

test("local Whisper auto-detects speech unless spoken languages are selected", async () => {
  fixture.sessionId = "offline-recording";
  fixture.provider = {
    provider: "local_whisper",
    model: "whisper-small-q8_0",
  };
  fixture.pending = true;
  fixture.localCalls.length = 0;
  fixture.requests = 0;
  fixture.summaries.length = 0;

  await transcribeSession(fixture.sessionId);

  assert.deepEqual(fixture.localCalls[0].params, {
    languages: ["vi"],
    keywords: ["Cloudflare"],
  });
  assert.equal(fixture.requests, 0);
  assert.equal(fixture.pending, false);
  assert.deepEqual(fixture.summaries, []);
  assert.equal(fixture.checkpointsDeleted.length, 1);

  fixture.sessionId = "offline-autodetect-recording";
  fixture.pending = true;
  fixture.localCalls.length = 0;
  fixture.preferences = {
    ai_language: "en",
    spoken_languages: [],
    personalization_dictionary_terms: [],
  };
  await transcribeSession(fixture.sessionId);
  assert.deepEqual(fixture.localCalls[0].params.languages, []);
  assert.equal(fixture.pending, false);
  assert.equal(fixture.checkpointsDeleted.length, 2);
});

test("local transcription checkpoint survives inference and database failures", async () => {
  fixture.provider = {
    provider: "local_whisper",
    model: "whisper-small-q8_0",
  };
  for (const failure of ["inference", "database"]) {
    fixture.sessionId = `local-checkpoint-${failure}-retry`;
    fixture.pending = true;
    fixture.localError =
      failure === "inference" ? new Error("Interrupted local inference") : null;
    fixture.transactionError =
      failure === "database" ? new Error("Temporary database failure") : null;
    const deletedBeforeFailure = fixture.checkpointsDeleted.length;

    await transcribeSession(fixture.sessionId);
    assert.equal(fixture.pending, true);
    assert.equal(fixture.checkpointsDeleted.length, deletedBeforeFailure);

    fixture.localError = null;
    fixture.transactionError = null;
    await retryPendingTranscriptions();
    assert.equal(fixture.pending, false);
    assert.equal(fixture.checkpointsDeleted.length, deletedBeforeFailure + 1);
  }
});
