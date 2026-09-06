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
  resolutions: 0,
  requests: 0,
  transactions: [],
  failures: [],
  summaries: [],
  requestError: null,
});
const mocks = {
  "./summarize": `export function generateSummaryAfterTranscription(sessionId) { globalThis.transcriptionRetryFixture.summaries.push(sessionId); }`,
  "expo-file-system": `export const Paths = {document: '/synthetic'}; export class File { exists = true; size = 1024; }`,
  "expo/fetch": `export function fetch() { throw new Error('Unexpected hosted request'); }`,
  "react-native": `export const Platform = {OS: 'ios'};`,
  "@/auth/billing": `export class ProRequiredError extends Error {}`,
  "@/db": `const f = globalThis.transcriptionRetryFixture;
    export async function execute(sql, params) {
      if (!f.pending) return [];
      if (sql.includes('SELECT attachment.session_id')) return [{session_id: f.sessionId}];
      return [{content_type: 'audio/wav', size_bytes: 1024, local_relative_path: 'audio.wav'}];
    }
    export async function executeTransaction(statements) { f.transactions.push(statements); f.pending = false; }
  `,
  "@/settings/providers": `export async function resolveProvider() { const f = globalThis.transcriptionRetryFixture; f.resolutions++; return f.provider; }`,
  "@/settings/preferences": `export async function readPreferences() { return {ai_language: 'en', spoken_languages: [], personalization_dictionary_terms: []}; }`,
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
