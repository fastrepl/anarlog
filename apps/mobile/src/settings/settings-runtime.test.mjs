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
  providersFor,
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
  "@anlg/provider-validation": `export async function verifyProviderCredentials(credential) { return globalThis.mobileSettingsFixture.verify(credential); }`,
  "@anlg/mobile-bridge": `export const ProviderTranscriptionError = Object.fromEntries(["AudioTooLarge", "AudioMissing", "ResponseTooLarge", "InvalidSettings", "TimedOut", "RequestFailed"].map(tag => [tag, {instanceOf: error => error.tag === tag}]));
  export async function transcribeProviderAudio(request, options) {
    const fixture = globalThis.mobileSettingsFixture;
    fixture.nativeRequest = {request: JSON.parse(request), signal: options.signal};
    options.signal.throwIfAborted();
    if (fixture.nativeError) throw fixture.nativeError;
    return JSON.stringify({status: 200, body: JSON.stringify({results: {channels: [{alternatives: [{transcript: 'Hello', words: []}]}]}})});
  }`,
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
const { createProviderAutosave } = await import("./provider-autosave.ts");
const {
  saveProviderConfig,
  saveProviderSetup,
  saveProviderConnection,
  readProviderConfig,
  readProviderSetup,
  readProviderStatus,
  removeProviderKey,
  resolveProvider,
} = await import("./providers.ts");
const { requestProviderTranscription } =
  await import("../data/provider-transcription.ts");
const { summarizeSession } = await import("../data/summarize.ts");
const { Platform } = await import("react-native");

function signedInWith(claims) {
  fixture.session = {
    user: { id: "account-a" },
    access_token: `synthetic.${Buffer.from(JSON.stringify(claims)).toString("base64url")}.signature`,
  };
}

beforeEach(() => {
  Platform.OS = "ios";
  fixture.nativeRequest = null;
  fixture.nativeError = null;
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
  fixture.verify = async () => {};
  fixture.respond = () =>
    Response.json({
      choices: [{ message: { content: "## Decisions\nShip the mobile app." } }],
    });
});

test("rejected provider keys never replace working credentials or settings", async () => {
  for (const kind of ["stt", "llm"]) {
    const config = defaultProviderConfig(kind, "openai");
    await saveProviderConnection("account-a", kind, config, "working-key");
    const before = [...fixture.keys.entries()];
    fixture.verify = async () => {
      throw Error("The provider rejected this key.");
    };
    await assert.rejects(
      saveProviderConnection("account-a", kind, config, "invalid-key"),
      /rejected/,
    );
    assert.deepEqual([...fixture.keys.entries()], before);
    assert.equal(
      (await readProviderStatus("account-a", kind, "openai")).isConfigured,
      false,
    );
    fixture.verify = async () => {};
    assert.equal(
      (await readProviderStatus("account-a", kind, "openai")).isConfigured,
      true,
    );
  }
});

test("failed verification never creates a provider or exposes it in selection", async () => {
  fixture.verify = async () => {
    throw Error("Couldn’t verify this key. Check your connection.");
  };
  await assert.rejects(
    saveProviderConnection(
      null,
      "stt",
      defaultProviderConfig("stt", "deepgram"),
      "bad-key",
    ),
    /Couldn’t verify/,
  );
  assert.equal(fixture.keys.size, 0);
  assert.equal(
    (await readProviderStatus(null, "stt", "deepgram")).isConfigured,
    false,
  );
  assert.equal((await readProviderConfig(null, "stt")).provider, "anarlog");
});

test("provider availability follows saved device keys for transcription and summaries", async () => {
  for (const kind of ["stt", "llm"]) {
    for (const { id } of providersFor(kind)) {
      const status = await readProviderStatus("account-a", kind, id);
      assert.equal(status.isConfigured, id === "anarlog", `${kind}/${id}`);
    }
    const config = defaultProviderConfig(kind, "openai");
    await saveProviderConnection("account-a", kind, config, "synthetic-key");
    const configured = await readProviderStatus("account-a", kind, "openai");
    assert.equal(configured.isConfigured, true);
    assert.equal(configured.hasKey, true);
    assert.equal("apiKey" in configured, false);
    assert.equal(
      (await readProviderStatus("account-b", kind, "openai")).isConfigured,
      false,
    );

    await removeProviderKey("account-a", kind, "openai");
    assert.equal(
      (await readProviderStatus("account-a", kind, "openai")).isConfigured,
      false,
    );
  }
});

test("provider availability rejects invalid saved keys and incomplete custom connections", async () => {
  for (const kind of ["stt", "llm"]) {
    for (const value of ["", "  ", "bad\nkey", "x".repeat(8193)]) {
      fixture.keys.set(providerStorageKey("account-a", kind, "openai"), {
        value,
      });
      assert.equal(
        (await readProviderStatus("account-a", kind, "openai")).isConfigured,
        false,
      );
    }
    fixture.keys.set(providerStorageKey("account-a", kind, "custom"), {
      value: "synthetic-key",
    });
    assert.equal(
      (await readProviderStatus("account-a", kind, "custom")).isConfigured,
      false,
    );
    await saveProviderConnection("account-a", kind, {
      provider: "custom",
      baseUrl: "https://custom.test/v1",
    });
    assert.equal(
      (await readProviderStatus("account-a", kind, "custom")).isConfigured,
      true,
    );
  }
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

test("saving provider credentials leaves the active selection unchanged", async () => {
  const config = {
    ...defaultProviderConfig("llm", "openai"),
    model: "saved-openai-model",
  };
  await saveProviderSetup("account-a", "llm", config, "synthetic-key");
  assert.equal(
    (await readProviderConfig("account-a", "llm")).provider,
    "anarlog",
  );
  assert.deepEqual(
    await readProviderSetup("account-a", "llm", "openai"),
    config,
  );
  assert.equal(
    fixture.keys.get(providerStorageKey("account-a", "llm", "openai")).value,
    "synthetic-key",
  );
  assert.equal(
    (await readProviderSetup("account-b", "llm", "openai")).model,
    "",
  );
  assert.equal(
    (await readProviderConfig("account-a", "stt")).provider,
    "anarlog",
  );
  assert.equal(fixture.requests.length, 0);
});

test("credentials can be saved before choosing a summary model", async () => {
  const connection = defaultProviderConfig("llm", "openai");
  await saveProviderConnection("account-a", "llm", connection, "synthetic-key");
  assert.equal(
    (await readProviderSetup("account-a", "llm", "openai")).model,
    "",
  );
  assert.equal(
    (await readProviderConfig("account-a", "llm")).provider,
    "anarlog",
  );
  assert.equal(
    fixture.keys.get(providerStorageKey("account-a", "llm", "openai")).value,
    "synthetic-key",
  );
  await assert.rejects(
    saveProviderConfig("account-a", "llm", connection),
    /model ID/,
  );

  await saveProviderConfig("account-a", "llm", {
    ...connection,
    model: "chosen-model",
  });
  signedInWith({});
  assert.equal((await resolveProvider("llm")).model, "chosen-model");
  assert.equal(fixture.requests.length, 0);
});

test("credential edits preserve the latest model instead of restoring a stale form value", async () => {
  const earlier = {
    ...defaultProviderConfig("llm", "custom"),
    baseUrl: "https://old.test/v1",
    model: "first-model",
  };
  await saveProviderConfig("account-a", "llm", earlier, "synthetic-old-key");
  await saveProviderConfig("account-a", "llm", {
    ...earlier,
    model: "new-model",
  });
  await saveProviderConnection(
    "account-a",
    "llm",
    { ...earlier, baseUrl: "https://new.test/v1" },
    "synthetic-new-key",
  );
  const active = await readProviderConfig("account-a", "llm");
  assert.equal(active.model, "new-model");
  assert.equal(active.baseUrl, "https://new.test/v1");
});

test("saving another provider's connection cannot change the active model", async () => {
  const active = {
    ...defaultProviderConfig("llm", "openai"),
    model: "active-model",
  };
  await saveProviderConfig("account-a", "llm", active, "synthetic-openai-key");
  await saveProviderConnection(
    "account-a",
    "llm",
    defaultProviderConfig("llm", "anthropic"),
    "synthetic-anthropic-key",
  );
  assert.deepEqual(await readProviderConfig("account-a", "llm"), active);
  assert.equal(
    (await readProviderSetup("account-a", "llm", "anthropic")).model,
    "",
  );
});

test("connection autosave accepts credentials without a model and rejects incomplete connections", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const writes = [];
  const autosave = createProviderAutosave(
    "llm",
    (draft) =>
      writes.push(
        saveProviderConnection("account-a", "llm", draft.config, draft.apiKey),
      ),
    { connectionOnly: true },
  );
  const config = {
    ...defaultProviderConfig("llm", "custom"),
    baseUrl: "https://custom.test/v1",
  };
  autosave.schedule(config, "synthetic-key", false);
  t.mock.timers.tick(500);
  await Promise.all(writes);
  assert.equal(writes.length, 1);
  assert.equal(
    (await readProviderSetup("account-a", "llm", "custom")).model,
    "",
  );
  for (const baseUrl of [
    "",
    "http://custom.test",
    "https://user:secret@custom.test",
  ]) {
    autosave.schedule({ ...config, baseUrl }, "synthetic-key", true);
    autosave.flush();
  }
  assert.equal(writes.length, 1);
});

test("switching providers restores each saved model and endpoint without re-entering keys", async () => {
  const openai = {
    ...defaultProviderConfig("llm", "openai"),
    model: "openai-model",
  };
  const custom = {
    provider: "custom",
    baseUrl: "https://private-provider.test/v1",
    model: "custom-model",
  };
  await saveProviderSetup("account-a", "llm", openai, "synthetic-openai-key");
  await saveProviderSetup("account-a", "llm", custom, "synthetic-custom-key");
  for (const config of [openai, custom, openai]) {
    await saveProviderConfig(
      "account-a",
      "llm",
      await readProviderSetup("account-a", "llm", config.provider),
    );
    assert.deepEqual(await readProviderConfig("account-a", "llm"), config);
  }
  assert.equal(fixture.keys.size, 2);
});

test("editing an active provider updates its runtime config while editing another does not", async () => {
  const active = {
    ...defaultProviderConfig("llm", "openai"),
    model: "first-model",
  };
  await saveProviderConfig("account-a", "llm", active, "synthetic-openai-key");
  const updated = { ...active, model: "updated-model" };
  await saveProviderSetup("account-a", "llm", updated);
  await saveProviderSetup(
    "account-a",
    "llm",
    { ...defaultProviderConfig("llm", "anthropic"), model: "anthropic-model" },
    "synthetic-anthropic-key",
  );
  assert.deepEqual(await readProviderConfig("account-a", "llm"), updated);
  signedInWith({});
  assert.equal((await resolveProvider("llm")).model, "updated-model");
});

test("legacy active settings survive the first provider switch, including edits made by older builds", async () => {
  const legacy = {
    ...defaultProviderConfig("stt", "deepgram"),
    model: "legacy-model",
  };
  const activeId = providerStorageKey("account-a", "stt");
  fixture.db
    .prepare("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
    .run(activeId, JSON.stringify(legacy));
  fixture.db
    .prepare("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
    .run(
      providerStorageKey("account-a", "stt", "deepgram"),
      JSON.stringify({ ...legacy, model: "outdated-model" }),
    );
  fixture.keys.set(providerStorageKey("account-a", "stt", "deepgram"), {
    value: "synthetic-existing-key",
  });
  assert.deepEqual(
    await readProviderSetup("account-a", "stt", "deepgram"),
    legacy,
  );
  await saveProviderConfig("account-a", "stt", defaultProviderConfig("stt"));
  const restored = await readProviderSetup("account-a", "stt", "deepgram");
  assert.deepEqual(restored, legacy);
  await saveProviderConfig("account-a", "stt", restored);
  assert.deepEqual(await readProviderConfig("account-a", "stt"), legacy);
});

test("invalid setup or a removed key cannot replace the current provider", async () => {
  const config = defaultProviderConfig("stt", "deepgram");
  await assert.rejects(
    saveProviderSetup("account-a", "stt", config),
    /Enter an API key/,
  );
  assert.equal(
    fixture.db
      .prepare(
        "SELECT count(*) AS count FROM app_settings WHERE id LIKE 'anarlog.provider.%'",
      )
      .get().count,
    0,
  );
  await saveProviderSetup("account-a", "stt", config, "synthetic-key");
  await removeProviderKey("account-a", "stt", "deepgram");
  await assert.rejects(
    saveProviderConfig("account-a", "stt", config),
    /Enter an API key/,
  );
  assert.equal(
    (await readProviderConfig("account-a", "stt")).provider,
    "anarlog",
  );
  assert.deepEqual(
    await readProviderSetup("account-a", "stt", "deepgram"),
    config,
  );
});

test("provider autosave waits for typing to stop and persists only the latest valid draft", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const writes = [];
  const autosave = createProviderAutosave("llm", (draft) => {
    writes.push(
      saveProviderSetup("account-a", "llm", draft.config, draft.apiKey),
    );
  });
  const config = {
    ...defaultProviderConfig("llm", "openai"),
    model: "first-model",
  };
  autosave.schedule(config, "synthetic-first-key", false);
  t.mock.timers.tick(300);
  const latest = { ...config, model: "latest-model" };
  autosave.schedule(latest, "synthetic-latest-key", false);
  t.mock.timers.tick(499);
  assert.equal(writes.length, 0);
  t.mock.timers.tick(1);
  await Promise.all(writes);
  assert.equal(writes.length, 1);
  assert.deepEqual(
    await readProviderSetup("account-a", "llm", "openai"),
    latest,
  );
  assert.equal(
    fixture.keys.get(providerStorageKey("account-a", "llm", "openai")).value,
    "synthetic-latest-key",
  );
  assert.equal(
    (await readProviderConfig("account-a", "llm")).provider,
    "anarlog",
  );
});

test("an incomplete or invalid edit cancels the pending provider autosave", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const writes = [];
  const autosave = createProviderAutosave("llm", (draft) => writes.push(draft));
  const config = {
    provider: "custom",
    baseUrl: "https://provider.test/v1",
    model: "custom-model",
  };
  for (const [draft, key] of [
    [{ ...config, model: "" }, "synthetic-key"],
    [{ ...config, model: "line\nbreak" }, "synthetic-key"],
    [{ ...config, baseUrl: "http://provider.test/v1" }, "synthetic-key"],
    [
      { ...config, baseUrl: "https://user:password@provider.test" },
      "synthetic-key",
    ],
    [config, ""],
    [config, "line\nbreak"],
    [config, "a".repeat(8193)],
  ]) {
    autosave.schedule(config, "synthetic-valid-key", false);
    autosave.schedule(draft, key, false);
    autosave.flush();
    t.mock.timers.tick(500);
    assert.equal(writes.length, 0);
  }
});

test("autosave flushes on leaving a field exactly once and reuses a saved key", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const config = defaultProviderConfig("stt", "deepgram");
  await saveProviderConfig(
    "account-a",
    "stt",
    config,
    "synthetic-existing-key",
  );
  const writes = [];
  const autosave = createProviderAutosave("stt", (draft) => {
    writes.push(
      saveProviderSetup("account-a", "stt", draft.config, draft.apiKey),
    );
  });
  const edited = { ...config, model: "  edited-model  " };
  autosave.schedule(edited, "", true);
  autosave.flush();
  autosave.flush();
  t.mock.timers.tick(500);
  await Promise.all(writes);
  assert.equal(writes.length, 1);
  assert.equal(
    (await readProviderConfig("account-a", "stt")).model,
    "edited-model",
  );
  assert.equal(
    fixture.keys.get(providerStorageKey("account-a", "stt", "deepgram")).value,
    "synthetic-existing-key",
  );
});

test("canceling a pending autosave does not restore a removed provider key", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const config = defaultProviderConfig("stt", "deepgram");
  await saveProviderSetup("account-a", "stt", config, "synthetic-existing-key");
  const writes = [];
  const autosave = createProviderAutosave("stt", (draft) => writes.push(draft));
  autosave.schedule(config, "synthetic-replacement-key", true);
  autosave.cancel();
  await removeProviderKey("account-a", "stt", "deepgram");
  autosave.flush();
  t.mock.timers.tick(500);
  assert.equal(writes.length, 0);
  assert.equal(fixture.keys.size, 0);
  assert.deepEqual(
    await readProviderSetup("account-a", "stt", "deepgram"),
    config,
  );
});

test("invalid autosaved edits preserve the last working provider configuration", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const config = defaultProviderConfig("stt", "deepgram");
  await saveProviderConfig(
    "account-a",
    "stt",
    config,
    "synthetic-existing-key",
  );
  const writes = [];
  const autosave = createProviderAutosave("stt", (draft) => {
    writes.push(
      saveProviderSetup("account-a", "stt", draft.config, draft.apiKey),
    );
  });
  autosave.schedule({ ...config, model: "" }, "", true);
  t.mock.timers.tick(500);
  assert.equal(writes.length, 0);
  assert.deepEqual(await readProviderConfig("account-a", "stt"), config);
  assert.equal(fixture.requests.length, 0);
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
  Platform.OS = "web";
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
  Platform.OS = "web";
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
  Platform.OS = "web";
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

for (const kind of ["stt", "llm"]) {
  for (const definition of providersFor(kind).filter(
    ({ id }) => id !== "anarlog",
  )) {
    test(`${kind} ${definition.name} keeps its key on this device and restores its own setup`, async () => {
      const config = {
        ...defaultProviderConfig(kind, definition.id),
        baseUrl: definition.baseUrl || "https://gateway.example/v1",
        model: definition.model || "test-model",
      };
      await saveProviderSetup("account-a", kind, config, "synthetic-key");
      assert.deepEqual(
        await readProviderSetup("account-a", kind, definition.id),
        config,
      );
      assert.equal(
        (await readProviderConfig("account-a", kind)).provider,
        "anarlog",
      );
      await saveProviderConfig("account-a", kind, config);
      assert.deepEqual(await readProviderConfig("account-a", kind), config);
      assert.equal(
        fixture.keys.get(providerStorageKey("account-a", kind, definition.id))
          .options.keychainAccessible,
        "device-only",
      );
      assert.ok(
        !fixture.db
          .prepare("SELECT value_json FROM app_settings")
          .all()
          .some((row) => row.value_json.includes("synthetic-key")),
      );
      assert.deepEqual(
        await readProviderSetup("account-b", kind, definition.id),
        defaultProviderConfig(kind, definition.id),
      );
      await removeProviderKey("account-a", kind, definition.id);
      assert.ok(
        !fixture.keys.has(providerStorageKey("account-a", kind, definition.id)),
      );
    });
  }
}

for (const definition of providersFor("stt").filter(
  ({ id }) => !["anarlog", "custom"].includes(id),
)) {
  test(`${definition.name} delegates native audio to the desktop adapter with cancellation`, async () => {
    const controller = new AbortController();
    const response = await requestProviderTranscription(
      { uri: "file:///documents/sessions/test/audio.wav", size: 100 },
      "audio/wav",
      {
        ...defaultProviderConfig("stt", definition.id),
        apiKey: "synthetic-key",
      },
      {
        ...DEFAULT_PREFERENCES,
        spoken_languages: ["ko", "en"],
        personalization_dictionary_terms: ["Anarlog"],
      },
      controller.signal,
    );
    assert.equal(fixture.nativeRequest.request.provider, definition.id);
    assert.equal(
      fixture.nativeRequest.request.file_uri,
      "file:///documents/sessions/test/audio.wav",
    );
    assert.equal(fixture.nativeRequest.request.api_key, "synthetic-key");
    assert.equal(fixture.nativeRequest.signal, controller.signal);
    assert.deepEqual(fixture.nativeRequest.request.params.languages, [
      "ko",
      "en",
    ]);
    assert.deepEqual(fixture.nativeRequest.request.params.keywords, [
      "Anarlog",
    ]);
    assert.equal(fixture.requests.length, 0);
    assert.equal(
      normalizeTranscriptionResponse(definition.id, JSON.parse(response.body))
        .results.channels[0].alternatives[0].transcript,
      "Hello",
    );
  });
}

test("native transcription preserves an already aborted signal", async () => {
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(
    requestProviderTranscription(
      { uri: "file:///audio.wav", size: 100 },
      "audio/wav",
      {
        ...defaultProviderConfig("stt", "assemblyai"),
        apiKey: "synthetic-key",
      },
      DEFAULT_PREFERENCES,
      controller.signal,
    ),
    { name: "AbortError" },
  );
});

for (const definition of providersFor("llm").filter(
  ({ id }) => id !== "anarlog",
)) {
  test(`${definition.name} generates and persists a summary with its authentication format`, async () => {
    createNote();
    const config = {
      ...defaultProviderConfig("llm", definition.id),
      baseUrl: definition.baseUrl || "https://gateway.example/v1",
      model: "test-model",
    };
    await saveProviderConfig(null, "llm", config, "synthetic-key");
    fixture.respond = () =>
      Response.json(
        definition.id === "anthropic"
          ? { content: [{ type: "text", text: "Summary" }] }
          : definition.id === "google_generative_ai"
            ? {
                candidates: [
                  {
                    content: {
                      parts: [
                        { thought: true, text: "Private reasoning" },
                        { text: "Summary" },
                      ],
                    },
                  },
                ],
              }
            : { choices: [{ message: { content: "Summary" } }] },
      );
    await summarizeSession("note-1");
    const { url, options } = fixture.requests[0];
    assert.equal(new URL(url).origin, new URL(config.baseUrl).origin);
    assert.equal(options.redirect, "error");
    assert.ok(options.signal instanceof AbortSignal);
    const body = JSON.parse(options.body);
    if (definition.id === "anthropic") {
      assert.ok(url.endsWith("/messages"));
      assert.equal(options.headers["x-api-key"], "synthetic-key");
      assert.equal(options.headers["anthropic-version"], "2023-06-01");
      assert.ok(body.system);
    } else if (definition.id === "google_generative_ai") {
      assert.ok(url.endsWith("/models/test-model:generateContent"));
      assert.equal(options.headers["x-goog-api-key"], "synthetic-key");
      assert.ok(body.systemInstruction.parts[0].text);
      assert.ok(body.contents[0].parts[0].text.includes("Ship the app"));
    } else {
      assert.ok(url.endsWith("/chat/completions"));
      assert.equal(body.model, "test-model");
      assert.equal(
        options.headers[
          definition.id === "azure_openai" ? "api-key" : "Authorization"
        ],
        definition.id === "azure_openai"
          ? "synthetic-key"
          : "Bearer synthetic-key",
      );
      if (definition.id === "azure_ai")
        assert.equal(options.headers["api-key"], "synthetic-key");
    }
    assert.equal(
      fixture.db
        .prepare("SELECT body FROM session_documents WHERE kind = 'summary'")
        .get().body,
      "Summary",
    );
  });
}

test("native file and response limits remain permanent failures", async () => {
  for (const [tag, code] of [
    ["AudioTooLarge", "audio_too_large"],
    ["AudioMissing", "audio_missing"],
    ["ResponseTooLarge", "stt_response_too_large"],
  ]) {
    fixture.nativeError = { tag, inner: { maxMegabytes: 25n } };
    await assert.rejects(
      requestProviderTranscription(
        { uri: "file:///audio.wav", size: 100 },
        "audio/wav",
        { ...defaultProviderConfig("stt", "openai"), apiKey: "synthetic-key" },
        DEFAULT_PREFERENCES,
        new AbortController().signal,
      ),
      (error) => error.code === code,
    );
  }
});

test("Azure resource URLs use the v1 endpoint without duplicating its path", async () => {
  const { summaryRequest } = await import("../data/provider-summary.ts");
  for (const baseUrl of [
    "https://resource.openai.azure.com",
    "https://resource.openai.azure.com/openai",
    "https://resource.openai.azure.com/openai/v1",
  ]) {
    const request = summaryRequest(
      {
        provider: "azure_openai",
        baseUrl,
        model: "my-deployment",
        apiKey: "synthetic-key",
      },
      "Summarize",
      "Notes",
      "",
    );
    assert.equal(
      request.url,
      "https://resource.openai.azure.com/openai/v1/chat/completions",
    );
    assert.equal(request.body.model, "my-deployment");
    assert.equal(request.headers["api-key"], "synthetic-key");
    assert.ok(!request.headers.Authorization);
  }
});

const { discoverProviderModels, parseProviderModels } =
  await import("./provider-models.ts");
const { presetProviderModels } = await import("./provider-model-catalog.ts");

for (const definition of providersFor("llm").filter(
  ({ id }) => id !== "anarlog" && !presetProviderModels("llm", id),
)) {
  test(`${definition.name} discovers selectable models with its device key and authentication headers`, async () => {
    const config = {
      ...defaultProviderConfig("llm", definition.id),
      baseUrl: definition.baseUrl || "https://provider.example.test",
    };
    await saveProviderConnection(
      "account-a",
      "llm",
      config,
      "synthetic-discovery-key",
    );
    fixture.respond = () =>
      Response.json(
        definition.id === "google_generative_ai"
          ? {
              models: [
                {
                  name: "models/test-chat",
                  supportedGenerationMethods: ["generateContent"],
                },
              ],
            }
          : { data: [{ id: "test-chat" }] },
      );
    assert.deepEqual(
      await discoverProviderModels(
        "account-a",
        config,
        new AbortController().signal,
      ),
      ["test-chat"],
    );
    const { url, options } = fixture.requests[0];
    assert.equal(
      url,
      `${config.baseUrl}${definition.id === "azure_openai" ? "/openai/models?api-version=2024-10-21" : "/models"}`,
    );
    const header =
      definition.id === "google_generative_ai"
        ? "x-goog-api-key"
        : definition.id === "anthropic"
          ? "x-api-key"
          : definition.id.startsWith("azure_")
            ? "api-key"
            : "Authorization";
    assert.equal(
      options.headers[header],
      `${header === "Authorization" ? "Bearer " : ""}synthetic-discovery-key`,
    );
    assert.equal(options.redirect, "error");
    assert.ok(!url.includes("synthetic-discovery-key"));
    assert.equal(
      (await readProviderSetup("account-a", "llm", definition.id)).model,
      "",
    );
    assert.equal(
      (await readProviderConfig("account-a", "llm")).provider,
      "anarlog",
    );
  });
}

test("model discovery excludes non-text models and keeps custom IDs and Cohere model names", () => {
  assert.deepEqual(
    parseProviderModels("openrouter", {
      data: [
        { id: "chat-model" },
        { id: "chat-model" },
        { id: "my-custom-model" },
        { id: "embedding-model" },
        { id: "speech-transcribe" },
        { id: "vision-only", architecture: { output_modalities: ["image"] } },
        { id: "speech-only", architecture: { input_modalities: ["audio"] } },
        { id: "no-chat", capabilities: { completion_chat: false } },
        { id: "bad\nmodel" },
        { id: "a".repeat(201) },
        null,
      ],
    }),
    ["chat-model", "my-custom-model"],
  );
  assert.deepEqual(
    parseProviderModels("cohere", {
      models: [
        { name: "command-a", endpoints: ["chat"] },
        { name: "classify", endpoints: ["classify"] },
      ],
    }),
    ["command-a"],
  );
  assert.deepEqual(
    parseProviderModels("google_generative_ai", {
      models: [
        {
          name: "models/gemini-chat",
          supportedGenerationMethods: ["generateContent"],
        },
        {
          name: "models/embedding",
          supportedGenerationMethods: ["embedContent"],
        },
      ],
    }),
    ["gemini-chat"],
  );
  assert.throws(() => parseProviderModels("openai", {}), /Invalid model list/);
});

test("model discovery accepts provider object types and still excludes non-chat capabilities", () => {
  assert.deepEqual(
    parseProviderModels("anthropic", {
      data: [{ id: "claude-sonnet-4-6", type: "model" }],
    }),
    ["claude-sonnet-4-6"],
  );
  assert.deepEqual(
    parseProviderModels("mistral", {
      data: [
        {
          id: "mistral-small-latest",
          type: "base",
          capabilities: { completion_chat: true },
        },
        {
          id: "ft:my-model",
          type: "fine-tuned",
          capabilities: { completion_chat: true },
        },
        {
          id: "no-chat",
          type: "base",
          capabilities: { completion_chat: false },
        },
        { id: "vector-model", type: "embedding" },
      ],
    }),
    ["ft:my-model", "mistral-small-latest"],
  );
});

test("model discovery preserves manual selection on empty, malformed, oversized, or failed responses", async () => {
  const config = {
    ...defaultProviderConfig("llm", "openai"),
    model: "my-manual-model",
  };
  await saveProviderConfig("account-a", "llm", config, "synthetic-key");
  fixture.respond = () => Response.json({ data: [] });
  assert.deepEqual(
    await discoverProviderModels(
      "account-a",
      config,
      new AbortController().signal,
    ),
    [],
  );
  for (const response of [
    () => new Response("private-provider-error-body", { status: 401 }),
    () => new Response("not-json"),
    () =>
      Response.json(
        { data: [] },
        { headers: { "content-length": String(9 * 1024 * 1024) } },
      ),
    () => new Response("x".repeat(8 * 1024 * 1024 + 1)),
  ]) {
    fixture.respond = response;
    await assert.rejects(
      discoverProviderModels("account-a", config, new AbortController().signal),
      { message: "Couldn’t load models. Retry or enter a model ID." },
    );
    assert.equal(
      (await readProviderConfig("account-a", "llm")).model,
      "my-manual-model",
    );
  }
});

test("discovery never uses another account's key or a stale custom endpoint", async () => {
  const config = {
    ...defaultProviderConfig("llm", "custom"),
    baseUrl: "https://old.example.test",
  };
  await saveProviderConnection("account-a", "llm", config, "synthetic-key");
  await assert.rejects(
    discoverProviderModels(
      "account-b",
      { ...defaultProviderConfig("llm", "openai") },
      new AbortController().signal,
    ),
    /Add an API key/,
  );
  await saveProviderConnection(
    "account-a",
    "llm",
    { ...config, baseUrl: "https://new.example.test" },
    "replacement-key",
  );
  await assert.rejects(
    discoverProviderModels("account-a", config, new AbortController().signal),
    /connection changed/,
  );
  assert.equal(fixture.requests.length, 0);
});

test("discovery aborts requests when leaving the picker", async () => {
  const config = defaultProviderConfig("llm", "openai");
  await saveProviderConnection("account-a", "llm", config, "synthetic-key");
  const before = new AbortController();
  before.abort();
  await assert.rejects(
    discoverProviderModels("account-a", config, before.signal),
    { name: "AbortError" },
  );
  assert.equal(fixture.requests.length, 0);
  const during = new AbortController();
  fixture.respond = (_url, { signal }) => {
    during.abort();
    assert.equal(signal.aborted, true);
    throw new Error("aborted");
  };
  await assert.rejects(
    discoverProviderModels("account-a", config, during.signal),
    /Couldn’t load models/,
  );
});

for (const suffix of ["", "/openai", "/openai/v1"]) {
  test(`Azure model discovery normalizes the configured endpoint ${suffix || "root"}`, async () => {
    const config = {
      ...defaultProviderConfig("llm", "azure_openai"),
      baseUrl: `https://azure.example.test${suffix}`,
    };
    await saveProviderConnection("account-a", "llm", config, "synthetic-key");
    fixture.respond = () => Response.json({ data: [{ id: "my-deployment" }] });
    await discoverProviderModels(
      "account-a",
      config,
      new AbortController().signal,
    );
    assert.equal(
      fixture.requests[0].url,
      "https://azure.example.test/openai/models?api-version=2024-10-21",
    );
  });
}
