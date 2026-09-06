import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { beforeEach, test } from "node:test";
import { fileURLToPath } from "node:url";

import { AppLockController } from "./app-lock-model.ts";
import { languageName } from "./languages.ts";
import {
  DEFAULT_PREFERENCES,
  parsePreferences,
  normalizeDictionary,
} from "./preferences-model.ts";
import {
  defaultProviderConfig,
  providerStorageKey,
  validateProviderConfig,
  normalizeTranscriptionResponse,
} from "./providers-model.ts";

const sourceRoot = new URL("../", import.meta.url);
const fixture = (globalThis.mobileSettingsFixture = {
  db: null,
  keys: new Map(),
  storage: new Map(),
  sentry: { starts: 0, closes: 0, client: null, nativeEnabled: false },
  session: null,
  requests: [],
  respond: () =>
    Response.json({
      choices: [{ message: { content: "## Decisions\nShip the mobile app." } }],
    }),
});
const modules = {
  "@/db": `const fixture = globalThis.mobileSettingsFixture;
    export async function execute(sql, params = []) { return fixture.db.prepare(sql).all(...params); }
    export async function executeTransaction(statements) {
      fixture.db.exec('BEGIN');
      try { const results = statements.map(({sql, params}) => fixture.db.prepare(sql).run(...params).changes); fixture.db.exec('COMMIT'); return results; }
      catch (error) { fixture.db.exec('ROLLBACK'); throw error; }
    }
    export function useLiveQuery() { throw new Error('Unexpected React hook in runtime test'); }`,
  "@/auth/client": `export const supabase = { auth: { getSession: async () => ({ data: { session: globalThis.mobileSettingsFixture.session } }) } };`,
  "@/lib/env": `export const env = { apiUrl: 'https://hosted.anarlog.test', appUrl: 'https://anarlog.test', sentryDsn: 'synthetic-dsn', posthogApiKey: 'synthetic-analytics-key', posthogHost: 'https://analytics.test' };`,
  "@/lib/ids": `import { randomUUID } from 'node:crypto'; export const id = randomUUID; export const nowIso = () => new Date().toISOString();`,
  "expo-secure-store": `const fixture = globalThis.mobileSettingsFixture;
    export const WHEN_UNLOCKED_THIS_DEVICE_ONLY = 'device-only';
    export async function getItemAsync(key) { return fixture.keys.get(key)?.value ?? null; }
    export async function setItemAsync(key, value, options) { fixture.keys.set(key, {value, options}); }
    export async function deleteItemAsync(key) { fixture.keys.delete(key); }`,
  "expo/fetch": `export async function fetch(url, options) {
    const fixture = globalThis.mobileSettingsFixture;
    fixture.requests.push({url, options});
    return fixture.respond(url, options);
  }`,
  "@react-native-async-storage/async-storage": `export default {
    getItem: async key => globalThis.mobileSettingsFixture.storage.get(key) ?? null,
    setItem: async (key, value) => { globalThis.mobileSettingsFixture.storage.set(key, value); },
    removeItem: async key => { globalThis.mobileSettingsFixture.storage.delete(key); }
  };`,
  "expo-constants": `export default {expoConfig: {version: 'test'}};`,
  "expo-crypto": `export { randomUUID } from 'node:crypto';`,
  "react-native": `export const Platform = {OS: 'ios'}; export const AppState = { currentState: 'active', addEventListener: () => ({ remove() {} }) };`,
  "@sentry/react-native": `const state = globalThis.mobileSettingsFixture.sentry;
    export function init(options) { state.starts++; state.nativeEnabled = true; state.client = { getOptions: () => options }; }
    export function getClient() { return state.client; }
    export async function close() { state.closes++; state.nativeEnabled = false; }
  `,
};

registerHooks({
  resolve(specifier, context, next) {
    if (modules[specifier])
      return {
        url: `data:text/javascript,${encodeURIComponent(modules[specifier])}`,
        shortCircuit: true,
      };
    if (specifier.startsWith("@/"))
      return {
        url: new URL(`${specifier.slice(2)}.ts`, sourceRoot).href,
        shortCircuit: true,
      };
    if (
      specifier.startsWith(".") &&
      context.parentURL?.startsWith(sourceRoot.href)
    ) {
      const url = new URL(`${specifier}.ts`, context.parentURL);
      if (existsSync(fileURLToPath(url)))
        return { url: url.href, shortCircuit: true };
    }
    return next(specifier, context);
  },
});

const { setPreference, readPreferences } = await import("./preferences.ts");
const {
  saveProviderConfig,
  readProviderConfig,
  removeProviderKey,
  resolveProvider,
} = await import("./providers.ts");
const { requestProviderTranscription } =
  await import("../data/provider-transcription.ts");
const { summarizeSession } = await import("../data/summarize.ts");

function signedInWith(claims) {
  fixture.session = {
    user: { id: "account-a" },
    access_token: `synthetic.${Buffer.from(JSON.stringify(claims)).toString("base64url")}.signature`,
  };
}

beforeEach(() => {
  fixture.db?.close();
  fixture.db = new DatabaseSync(":memory:");
  for (const migration of [
    "20260710223922_canonical_data_model.sql",
    "20260810120000_synced_preferences.sql",
  ]) {
    fixture.db.exec(
      readFileSync(
        new URL(
          `../../../../crates/db-app/migrations/${migration}`,
          import.meta.url,
        ),
        "utf8",
      ),
    );
  }
  fixture.keys.clear();
  fixture.session = null;
  fixture.requests = [];
  fixture.respond = () =>
    Response.json({
      choices: [{ message: { content: "## Decisions\nShip the mobile app." } }],
    });
});

test("preference writes work offline and sync only supported preferences to the bound workspace", async () => {
  await setPreference("theme", "dark");
  assert.equal((await readPreferences()).theme, "dark");
  assert.equal(
    fixture.db.prepare("SELECT count(*) AS count FROM synced_preferences").get()
      .count,
    0,
  );
  fixture.db
    .prepare("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
    .run(
      "cloudsync_workspace_binding",
      JSON.stringify({ workspace_id: "workspace-a" }),
    );
  await setPreference("sidebar_show_tags", true);
  await setPreference("personalization_dictionary_terms", ["Anarlog", "John"]);
  assert.equal((await readPreferences()).sidebar_show_tags, true);
  assert.deepEqual((await readPreferences()).personalization_dictionary_terms, [
    "Anarlog",
    "John",
  ]);
  assert.deepEqual(
    fixture.db
      .prepare("SELECT id, workspace_id FROM synced_preferences")
      .all()
      .map((row) => ({ ...row })),
    [{ id: "sidebar_show_tags", workspace_id: "workspace-a" }],
  );
  fixture.db
    .prepare(
      "UPDATE synced_preferences SET workspace_id = ?, value_json = 'false'",
    )
    .run("workspace-b");
  assert.equal((await readPreferences()).sidebar_show_tags, true);
});

test("desktop list encoding and malformed preferences preserve usable defaults", () => {
  const preferences = parsePreferences([
    { id: "theme", value_json: '"dark"', source_rank: 0 },
    { id: "theme", value_json: '"light"', source_rank: 1 },
    {
      id: "spoken_languages",
      value_json: JSON.stringify(
        JSON.stringify(["ko", "ko", "en", "garbage!"]),
      ),
      source_rank: 0,
    },
    { id: "summary_length", value_json: '"unknown"', source_rank: 0 },
    { id: "sidebar_show_tags", value_json: "{broken", source_rank: 0 },
  ]);
  assert.equal(preferences.theme, "light");
  assert.deepEqual(preferences.spoken_languages, ["ko", "en"]);
  assert.equal(preferences.summary_length, "detailed");
  assert.equal(preferences.sidebar_show_tags, false);
  assert.deepEqual(
    normalizeDictionary([
      " Anarlog ",
      "anarlog",
      "Two  words",
      "",
      "x".repeat(101),
    ]),
    ["Anarlog", "Two words"],
  );
  assert.equal(languageName("ko"), "Korean");
});

test("provider endpoints reject insecure URLs and pin credentials to known provider hosts", () => {
  for (const baseUrl of [
    "http://example.test/v1",
    "https://user:secret@example.test",
    "https://example.test?token=foo",
    "https://example.test/#hash",
  ]) {
    assert.throws(() =>
      validateProviderConfig("llm", {
        provider: "custom",
        baseUrl,
        model: "model",
      }),
    );
  }
  assert.equal(
    validateProviderConfig("llm", {
      provider: "openai",
      baseUrl: "https://wrong.test",
      model: "model",
    }).baseUrl,
    "https://api.openai.com/v1",
  );
  assert.throws(
    () =>
      validateProviderConfig("llm", {
        ...defaultProviderConfig("llm", "openai"),
        model: "",
      }),
    /model ID/,
  );
  assert.throws(() => providerStorageKey("bad.account", "stt"));
});

test("provider keys stay in device-only secure storage, separated by account and task", async () => {
  const config = {
    ...defaultProviderConfig("llm", "openai"),
    model: "test-model",
  };
  await saveProviderConfig("account-a", "llm", config, "synthetic-key-a");
  assert.equal(
    fixture.keys.get(providerStorageKey("account-a", "llm", "openai")).options
      .keychainAccessible,
    "device-only",
  );
  assert.ok(
    !JSON.stringify(
      fixture.db.prepare("SELECT * FROM app_settings").all(),
    ).includes("synthetic-key"),
  );
  assert.equal(
    fixture.db.prepare("SELECT count(*) AS count FROM synced_preferences").get()
      .count,
    0,
  );
  assert.equal(
    (await readProviderConfig("account-b", "llm")).provider,
    "anarlog",
  );
  assert.equal(
    (await readProviderConfig("account-a", "stt")).provider,
    "anarlog",
  );
  fixture.session = {
    user: { id: "account-a" },
    access_token: "synthetic-session-token",
  };
  assert.equal((await resolveProvider("llm")).apiKey, "synthetic-key-a");
  await removeProviderKey("account-a", "llm", "openai");
  await assert.rejects(resolveProvider("llm"), /Add an API key/);
  assert.equal(fixture.requests.length, 0);
});

test("hosted Anarlog Pro requires a session token", async () => {
  fixture.session = null;
  await assert.rejects(resolveProvider("stt"), /Sign in to use Anarlog Pro/);
});

test("corrupt privacy JSON does not enable app lock", async () => {
  const { preferencesFromStoredValue } = await import("./privacy-store.ts");
  assert.deepEqual(preferencesFromStoredValue("{not-json"), {
    analytics: true,
    errorReports: true,
    appLock: false,
    corrupt: true,
  });
  assert.equal(preferencesFromStoredValue(null).corrupt, false);
  assert.equal(preferencesFromStoredValue('{"appLock":true}').appLock, true);
});

test("missing keys cannot activate a provider and selecting Pro uses only the account token", async () => {
  await assert.rejects(
    saveProviderConfig(null, "stt", defaultProviderConfig("stt", "openai")),
    /Enter an API key/,
  );
  assert.equal((await readProviderConfig(null, "stt")).provider, "anarlog");
  signedInWith({
    subscription_status: "active",
    entitlements: ["hyprnote_pro"],
  });
  assert.equal(
    (await resolveProvider("stt")).apiKey,
    fixture.session.access_token,
  );
});

test("active trials use hosted models; expired trials cannot send STT or summary requests", async () => {
  createNote();
  const trial = {
    subscription_status: "trialing",
    trial_end: Date.now() / 1000 + 21 * 86400,
    entitlements: ["hyprnote_pro"],
  };
  signedInWith(trial);
  assert.equal(
    (await resolveProvider("stt")).apiKey,
    fixture.session.access_token,
  );
  await summarizeSession("note-1");
  assert.equal(
    fixture.requests[0].url,
    "https://hosted.anarlog.test/llm/chat/completions",
  );
  fixture.requests = [];
  signedInWith({ ...trial, trial_end: Date.now() / 1000 - 1 });
  await assert.rejects(
    resolveProvider("stt"),
    /active Pro trial or subscription/,
  );
  await assert.rejects(
    summarizeSession("note-1"),
    /active Pro trial or subscription/,
  );
  assert.equal(fixture.requests.length, 0);
  assert.match(
    fixture.db
      .prepare("SELECT body FROM session_documents WHERE id = 'note-1'")
      .get().body,
    /Ship the app/,
  );
  assert.equal(
    fixture.db
      .prepare(
        "SELECT count(*) AS count FROM session_documents WHERE kind = 'summary'",
      )
      .get().count,
    1,
  );
});

test("expired trial users can keep using their own transcription and summary keys", async () => {
  createNote();
  signedInWith({
    subscription_status: "paused",
    entitlements: ["hyprnote_pro"],
  });
  await saveProviderConfig(
    "account-a",
    "stt",
    { ...defaultProviderConfig("stt", "openai"), model: "whisper-1" },
    "synthetic-stt-key",
  );
  await saveProviderConfig(
    "account-a",
    "llm",
    { ...defaultProviderConfig("llm", "openai"), model: "test-model" },
    "synthetic-llm-key",
  );
  assert.equal((await resolveProvider("stt")).apiKey, "synthetic-stt-key");
  await summarizeSession("note-1");
  assert.equal(
    fixture.requests[0].url,
    "https://api.openai.com/v1/chat/completions",
  );
  assert.equal(
    fixture.requests[0].options.headers.Authorization,
    "Bearer synthetic-llm-key",
  );
});

test("OpenAI-compatible audio goes directly to the chosen host as multipart, with language and dictionary", async () => {
  fixture.respond = () => Response.json({ text: "Hello Anarlog" });
  const audio = new File(["synthetic audio"], "audio.wav", {
    type: "audio/wav",
  });
  const response = await requestProviderTranscription(
    audio,
    "audio/wav",
    {
      ...defaultProviderConfig("stt", "openai"),
      apiKey: "synthetic-provider-key",
    },
    {
      ...DEFAULT_PREFERENCES,
      ai_language: "ko-KR",
      personalization_dictionary_terms: ["Anarlog"],
    },
    new AbortController().signal,
  );
  const { url, options } = fixture.requests[0];
  assert.equal(url, "https://api.openai.com/v1/audio/transcriptions");
  assert.equal(options.headers.Authorization, "Bearer synthetic-provider-key");
  assert.equal(options.redirect, "error");
  assert.equal(options.body.get("language"), "ko");
  assert.equal(options.body.get("prompt"), "Anarlog");
  assert.equal(options.body.get("file").name, "audio.wav");
  assert.equal(
    normalizeTranscriptionResponse("openai", JSON.parse(response.body)).results
      .channels[0].alternatives[0].transcript,
    "Hello Anarlog",
  );
});

test("Deepgram receives raw audio with its auth scheme and vocabulary options", async () => {
  fixture.respond = () => Response.json({ results: { channels: [] } });
  const audio = new File(["audio"], "audio.wav");
  await requestProviderTranscription(
    audio,
    "audio/wav",
    {
      ...defaultProviderConfig("stt", "deepgram"),
      apiKey: "synthetic-deepgram-key",
    },
    { ...DEFAULT_PREFERENCES, personalization_dictionary_terms: ["Anarlog"] },
    new AbortController().signal,
  );
  const { url, options } = fixture.requests[0];
  assert.equal(new URL(url).origin, "https://api.deepgram.com");
  assert.equal(new URL(url).searchParams.get("keyterm"), "Anarlog");
  assert.equal(options.headers.Authorization, "Token synthetic-deepgram-key");
  assert.equal(options.body, audio);
});

test("oversized provider audio is rejected before sending a request", async () => {
  await assert.rejects(
    requestProviderTranscription(
      { size: 26 * 1024 * 1024 },
      "audio/wav",
      { ...defaultProviderConfig("stt", "openai"), apiKey: "synthetic" },
      DEFAULT_PREFERENCES,
      new AbortController().signal,
    ),
    /25 MB/,
  );
  assert.equal(fixture.requests.length, 0);
});

function createNote() {
  fixture.db
    .prepare(
      "INSERT INTO sessions (id, workspace_id) VALUES ('note-1', 'workspace-a')",
    )
    .run();
  fixture.db
    .prepare(
      "INSERT INTO session_documents (id, session_id, kind, body_format, body) VALUES ('note-1', 'note-1', 'note', 'markdown', '# Planning\n\nShip the app next week.')",
    )
    .run();
}

test("summary generation uses the selected provider and persists a canonical summary without changing notes", async () => {
  createNote();
  await setPreference("ai_language", "ko");
  await setPreference("summary_length", "crisp");
  await saveProviderConfig(
    null,
    "llm",
    { ...defaultProviderConfig("llm", "anthropic"), model: "test-model" },
    "synthetic-anthropic-key",
  );
  fixture.respond = () =>
    Response.json({
      content: [{ type: "text", text: "## Decisions\nShip it." }],
    });
  await summarizeSession("note-1");
  const { url, options } = fixture.requests[0];
  assert.equal(url, "https://api.anthropic.com/v1/messages");
  assert.equal(options.headers["x-api-key"], "synthetic-anthropic-key");
  const request = JSON.parse(options.body);
  assert.match(request.system, /in ko/);
  assert.match(request.system, /Keep it brief/);
  const summary = fixture.db
    .prepare("SELECT * FROM session_documents WHERE kind = 'summary'")
    .get();
  assert.equal(summary.workspace_id, "workspace-a");
  assert.equal(summary.body_format, "markdown");
  assert.equal(
    JSON.parse(summary.generation_metadata_json).provider,
    "anthropic",
  );
  assert.match(
    fixture.db
      .prepare("SELECT body FROM session_documents WHERE id = 'note-1'")
      .get().body,
    /Ship the app next week/,
  );
});

test("a concurrent summary edit is preserved when a generation finishes", async () => {
  createNote();
  await saveProviderConfig(
    null,
    "llm",
    { ...defaultProviderConfig("llm", "openai"), model: "test-model" },
    "synthetic-key",
  );
  fixture.db
    .prepare(
      "INSERT INTO session_documents (id, session_id, kind, body, updated_at) VALUES ('summary-1', 'note-1', 'summary', 'Original', 'before')",
    )
    .run();
  fixture.respond = () => {
    fixture.db
      .prepare(
        "UPDATE session_documents SET body = 'Edited on desktop', updated_at = 'after' WHERE id = 'summary-1'",
      )
      .run();
    return Response.json({ choices: [{ message: { content: "Generated" } }] });
  };
  await assert.rejects(summarizeSession("note-1"), /note changed/);
  assert.equal(
    fixture.db
      .prepare("SELECT body FROM session_documents WHERE id = 'summary-1'")
      .get().body,
    "Edited on desktop",
  );
});

test("summary provider errors cannot overwrite an existing summary or expose the response body", async () => {
  createNote();
  await saveProviderConfig(
    null,
    "llm",
    { ...defaultProviderConfig("llm", "openai"), model: "test-model" },
    "synthetic-key",
  );
  fixture.respond = () =>
    new Response("sensitive provider response", { status: 401 });
  await assert.rejects(
    summarizeSession("note-1"),
    (error) =>
      /Check your provider/.test(error.message) &&
      !error.message.includes("sensitive"),
  );
  assert.equal(
    fixture.db
      .prepare(
        "SELECT count(*) AS count FROM session_documents WHERE kind = 'summary'",
      )
      .get().count,
    0,
  );
});

test("app lock covers inactive snapshots and requires authentication after backgrounding", () => {
  const lock = new AppLockController();
  lock.beginAuthentication();
  lock.appStateChanged("inactive");
  lock.appStateChanged("active");
  lock.finishAuthentication(true);
  assert.equal(lock.getSnapshot().locked, false);
  lock.appStateChanged("inactive");
  assert.equal(lock.getSnapshot().covered, true);
  assert.equal(lock.getSnapshot().locked, true);
  lock.appStateChanged("active");
  assert.equal(lock.getSnapshot().locked, true);
});

test("cancellation, concurrent auth, and backgrounding during authentication never unlock", () => {
  const lock = new AppLockController();
  assert.equal(lock.beginAuthentication(), true);
  assert.equal(lock.beginAuthentication(), false);
  lock.finishAuthentication(false);
  assert.equal(lock.getSnapshot().locked, true);
  lock.beginAuthentication();
  lock.appStateChanged("background");
  lock.appStateChanged("active");
  lock.finishAuthentication(true);
  assert.equal(lock.getSnapshot().locked, true);
});

test("turning off error reports disables the native SDK and privacy writes preserve other choices", async () => {
  globalThis.__DEV__ = false;
  const { initializeErrorReporting } =
    await import("../lib/error-reporting.ts");
  const { getPrivacyPreferences, setPrivacyPreference } =
    await import("./privacy-store.ts");
  await initializeErrorReporting();
  assert.equal(fixture.sentry.nativeEnabled, true);
  await setPrivacyPreference("errorReports", false);
  assert.equal(fixture.sentry.client.getOptions().enabled, false);
  await new Promise(setImmediate);
  assert.equal(fixture.sentry.nativeEnabled, false);
  await Promise.all([
    setPrivacyPreference("analytics", false),
    setPrivacyPreference("appLock", true),
  ]);
  assert.equal(getPrivacyPreferences().analytics, false);
  assert.equal(getPrivacyPreferences().appLock, true);
  assert.equal(fixture.sentry.starts, 1);
  await setPrivacyPreference("errorReports", true);
  await new Promise(setImmediate);
  assert.equal(fixture.sentry.starts, 2);
  assert.equal(fixture.sentry.nativeEnabled, true);
  assert.equal(
    JSON.parse(fixture.storage.get("anarlog:device-privacy")).analytics,
    false,
  );
});

test("analytics opt-out aborts in-flight requests and prevents subsequent events", async () => {
  globalThis.__DEV__ = false;
  const { captureAnalytics } = await import("../lib/analytics.ts");
  const { setPrivacyPreference } = await import("./privacy-store.ts");
  await setPrivacyPreference("analytics", true);
  const originalFetch = globalThis.fetch;
  let signal;
  let calls = 0;
  let started;
  const requestStarted = new Promise((resolve) => {
    started = resolve;
  });
  globalThis.fetch = async (_url, options) => {
    calls++;
    signal = options.signal;
    started();
    return new Promise((_resolve, reject) =>
      signal.addEventListener("abort", () => reject(new Error("aborted"))),
    );
  };
  try {
    captureAnalytics("settings_test");
    await requestStarted;
    await setPrivacyPreference("analytics", false);
    assert.equal(signal.aborted, true);
    captureAnalytics("must_not_send");
    await new Promise(setImmediate);
    assert.equal(calls, 1);
  } finally {
    globalThis.fetch = originalFetch;
  }
});
