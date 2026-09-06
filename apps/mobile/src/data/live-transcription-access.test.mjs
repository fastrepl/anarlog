import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import { beforeEach, test } from "node:test";

const root = new URL("../", import.meta.url);
const fixture = (globalThis.liveAccessFixture = {
  session: null,
  listeners: new Set(),
  sockets: [],
  failures: [],
  provider: null,
  native: [],
  transactions: [],
  apiKey: "synthetic-key",
});
const mocks = {
  "@anlg/mobile-bridge": `export async function startProviderLiveTranscription(json, listener, options) {
    const f = globalThis.liveAccessFixture;
    const native = {request: JSON.parse(json), listener, signal: options.signal, sent: [], cancelled: false,
      sendAudio(data) { if (f.backpressure) throw new Error('queue full'); this.sent.push(data); },
      async finish() { f.finalize?.(listener); return !f.finalizeFailed; },
      cancel() { this.cancelled = true; },
    };
    f.native.push(native);
    if (f.connectWait) await f.connectWait;
    return native;
  }`,
  "react-native": `export const Platform = { OS: "ios" };`,
  "@/auth/client": `const f = globalThis.liveAccessFixture; export const supabase = { auth: {
    getSession: async () => ({ data: { session: f.session } }),
    onAuthStateChange: listener => { f.listeners.add(listener); return { data: { subscription: { unsubscribe: () => f.listeners.delete(listener) } } }; }
  } };`,
  "@/db": `export async function execute() { return []; } export async function executeTransaction(statements) { globalThis.liveAccessFixture.transactions.push(statements); return statements.map(() => 1); }`,
  "@/settings/providers": `export async function readProviderConfig() { globalThis.liveAccessFixture.providerRead?.(); return globalThis.liveAccessFixture.provider ?? { provider: 'anarlog', model: 'cloud' }; } export async function readProviderKey() { return globalThis.liveAccessFixture.apiKey; }`,
  "@/settings/preferences": `export async function readPreferences() { return { ai_language: "en", spoken_languages: ["ko"], personalization_dictionary_terms: ["Anarlog"] }; }`,
  "@/settings/preferences-model": `export const applyTranscriptionPreferences = url => url;`,
  "@/lib/ids": `export const id = () => 'transcript-test'; export const nowIso = () => new Date().toISOString();`,
  "@/lib/env": `export const env = { apiUrl: 'https://api.anarlog.test' };`,
  "@/lib/analytics": `export function captureAnalytics() {}`,
  "@/lib/error-reporting": `export function captureOperationalError(error) { globalThis.liveAccessFixture.failures.push(error); }`,
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

const { SessionLiveTranscription } = await import("./live-transcription.ts");

function session(claims) {
  return {
    user: { id: "account-a" },
    access_token: `test.${Buffer.from(JSON.stringify(claims)).toString("base64url")}.signature`,
  };
}
function emitAuth(next) {
  fixture.session = next;
  for (const listener of fixture.listeners) listener("TOKEN_REFRESHED", next);
}
const settle = () => new Promise(setImmediate);

beforeEach(() => {
  fixture.session = null;
  fixture.listeners.clear();
  fixture.sockets = [];
  fixture.failures = [];
  fixture.provider = null;
  fixture.providerRead = null;
  fixture.apiKey = "synthetic-key";
  fixture.native = [];
  fixture.transactions = [];
  fixture.backpressure = false;
  fixture.finalizeFailed = false;
  fixture.finalize = null;
  fixture.connectWait = null;
  globalThis.WebSocket = class {
    static OPEN = 1;
    readyState = 0;
    bufferedAmount = 0;
    sent = [];
    constructor() {
      fixture.sockets.push(this);
    }
    open() {
      this.readyState = 1;
      this.onopen?.();
    }
    send(data) {
      this.sent.push(data);
    }
    close() {
      this.readyState = 3;
      this.onclose?.();
    }
  };
});

test("free and expired accounts record locally without opening a hosted socket or reporting a failure", async () => {
  for (const claims of [
    {},
    {
      subscription_status: "trialing",
      trial_end: Date.now() / 1000 - 1,
      entitlements: ["hyprnote_pro"],
    },
  ]) {
    fixture.session = session(claims);
    const updates = [];
    const live = new SessionLiveTranscription("note-a", 16000, 1, (update) =>
      updates.push(update),
    );
    await settle();
    live.sendAudio(new ArrayBuffer(16));
    assert.equal(updates.at(-1).status, "fallback");
    assert.equal(await live.stop(), false);
  }
  assert.equal(fixture.sockets.length, 0);
  assert.equal(fixture.failures.length, 0);
});

test("trial expiry closes an active socket and stops sending audio while preserving local recording", async (t) => {
  const now = Date.UTC(2026, 8, 6);
  t.mock.timers.enable({ apis: ["Date", "setTimeout", "setInterval"], now });
  fixture.session = session({
    subscription_status: "trialing",
    trial_end: (now + 2000) / 1000,
  });
  const updates = [];
  const live = new SessionLiveTranscription("note-a", 16000, 1, (update) =>
    updates.push(update),
  );
  await settle();
  const socket = fixture.sockets[0];
  socket.open();
  live.sendAudio(new ArrayBuffer(16));
  assert.equal(updates.at(-1).status, "live");
  t.mock.timers.tick(2000);
  live.sendAudio(new ArrayBuffer(16));
  assert.equal(socket.readyState, 3);
  assert.equal(socket.sent.length, 1);
  assert.equal(updates.at(-1).status, "fallback");
  assert.equal(fixture.listeners.size, 0);
  assert.equal(fixture.failures.length, 0);
  assert.equal(await live.stop(), false);
});

test("renewal during recording clears the trial deadline; sign-out still closes the connection", async (t) => {
  const now = Date.UTC(2026, 8, 6);
  t.mock.timers.enable({ apis: ["Date", "setTimeout", "setInterval"], now });
  fixture.session = session({
    subscription_status: "trialing",
    trial_end: (now + 2000) / 1000,
  });
  const live = new SessionLiveTranscription("note-a", 16000, 1, () => {});
  await settle();
  const socket = fixture.sockets[0];
  socket.open();
  emitAuth(
    session({ subscription_status: "active", entitlements: ["hyprnote_pro"] }),
  );
  t.mock.timers.tick(2000);
  live.sendAudio(new ArrayBuffer(16));
  assert.equal(socket.readyState, 1);
  assert.equal(socket.sent.length, 1);
  emitAuth(null);
  assert.equal(socket.readyState, 3);
  await live.stop();
  assert.equal(fixture.listeners.size, 0);
});

function providerSession(model = "universal-3-5-pro-realtime") {
  fixture.provider = {
    provider: "assemblyai",
    model,
    baseUrl: "https://api.assemblyai.com",
  };
  fixture.session = session({
    subscription_status: "trialing",
    trial_end: Date.now() / 1000 - 100,
  });
}
function transcript(text = "Hello", isFinal = true) {
  return JSON.stringify({
    type: "Results",
    is_final: isFinal,
    start: 0,
    duration: 1,
    channel_index: [0],
    channel: {
      alternatives: [
        {
          transcript: text,
          words: [{ word: text, start: 0, end: 1, speaker: 1 }],
        },
      ],
    },
  });
}

test("own-key live recording works after trial expiry and persists the selected provider and final words", async () => {
  providerSession();
  const updates = [];
  const live = new SessionLiveTranscription("note-a", 16000, 1, (update) =>
    updates.push(update),
  );
  const pcm = new ArrayBuffer(6400);
  live.sendAudio(pcm);
  await settle();
  const native = fixture.native[0];
  assert.equal(fixture.sockets.length, 0);
  assert.equal(native.request.provider, "assemblyai");
  assert.equal(native.request.api_key, "synthetic-key");
  assert.deepEqual(native.request.params, {
    model: "universal-3-5-pro-realtime",
    sample_rate: 16000,
    channels: 1,
    languages: ["en", "ko"],
    keywords: ["Anarlog"],
  });
  assert.deepEqual(native.sent, [pcm]);
  native.listener.onMessage(transcript("Hel", false));
  assert.equal(updates.at(-1).text, "Hel");
  assert.equal(fixture.transactions.length, 0);
  native.listener.onMessage(transcript("Hello"));
  await settle();
  assert.equal(
    updates.at(-1).text,
    "",
    "Committed words are read from SQLite, not repeated as a partial",
  );
  fixture.finalize = (listener) => listener.onMessage(transcript("Hello!"));
  assert.equal(await live.stop(), true);
  assert.equal(native.cancelled, true);
  const insert = fixture.transactions[0][1];
  assert.deepEqual(insert.params.slice(1, 3), [
    "assemblyai",
    "universal-3-5-pro-realtime",
  ]);
  const finish = fixture.transactions.at(-1)[0];
  assert.equal(JSON.parse(finish.params[0])[0].text, "Hello!");
  assert.equal(
    JSON.parse(JSON.parse(finish.params[1])[0].value).provider,
    "assemblyai",
  );
  assert.equal(fixture.failures.length, 0);
});

test("batch models and missing keys keep local recording without attempting a live connection", async () => {
  for (const [model, apiKey] of [
    ["universal-3-5-pro", "synthetic-key"],
    ["universal-3-5-pro-realtime", ""],
  ]) {
    providerSession(model);
    fixture.apiKey = apiKey;
    const live = new SessionLiveTranscription("note-a", 16000, 1, () => {});
    await settle();
    assert.equal(await live.stop(), false);
  }
  assert.equal(fixture.native.length, 0);
  assert.equal(fixture.failures.length, 0);
});

test("a full connect backlog of unaligned frames fits the native audio queue without losing PCM", async (t) => {
  providerSession();
  let connected;
  fixture.connectWait = new Promise((resolve) => {
    connected = resolve;
  });
  const updates = [];
  const live = new SessionLiveTranscription("note-a", 16000, 1, (update) =>
    updates.push(update),
  );
  t.after(() => live.stop());
  await settle();
  const native = fixture.native[0];
  const chunks = [];
  t.mock.method(native, "sendAudio", (buffer) => {
    for (let offset = 0; offset < buffer.byteLength; offset += 3200) {
      if (chunks.length === 100) throw new Error("queue full");
      chunks.push(Buffer.from(buffer.slice(offset, offset + 3200)));
    }
  });
  const pcm = Uint8Array.from({ length: 320_000 }, (_, i) => i % 251);
  for (let offset = 0; offset < pcm.byteLength; offset += 4096) {
    live.sendAudio(pcm.slice(offset, offset + 4096).buffer);
  }
  assert.equal(chunks.length, 0);

  connected();
  await settle();
  assert.equal(updates.at(-1).status, "live");
  assert.equal(native.cancelled, false);
  assert.equal(chunks.length, 100);
  assert.deepEqual(Buffer.concat(chunks), Buffer.from(pcm));
  assert.equal(fixture.failures.length, 0);
  native.listener.onMessage(transcript());
  assert.equal(await live.stop(), true);
});

test("sign-out cancels an own-key stream and ignores late provider callbacks", async () => {
  providerSession();
  const live = new SessionLiveTranscription("note-a", 16000, 1, () => {});
  await settle();
  const native = fixture.native[0];
  emitAuth(null);
  live.sendAudio(new ArrayBuffer(16));
  native.listener.onMessage(transcript());
  await settle();
  assert.equal(native.cancelled, true);
  assert.equal(native.sent.length, 0);
  assert.equal(fixture.transactions.length, 0);
  assert.equal(await live.stop(), false);
});

test("stopping while an own-key connection is pending cancels it without uploading queued audio", async () => {
  providerSession();
  let connected;
  fixture.connectWait = new Promise((resolve) => {
    connected = resolve;
  });
  const live = new SessionLiveTranscription("note-a", 16000, 1, () => {});
  live.sendAudio(new ArrayBuffer(16));
  await settle();
  const stop = live.stop();
  connected();
  assert.equal(await stop, false);
  assert.equal(fixture.native[0].signal.aborted, true);
  assert.equal(fixture.native[0].cancelled, true);
  assert.equal(fixture.native[0].sent.length, 0);
});

test("provider failure, backpressure, and incomplete finalization leave the recording for batch recovery", async () => {
  for (const failure of ["provider", "backpressure", "finalize"]) {
    providerSession();
    const live = new SessionLiveTranscription("note-a", 16000, 1, () => {});
    await settle();
    const native = fixture.native.at(-1);
    if (failure === "provider") native.listener.onError();
    if (failure === "backpressure") {
      fixture.backpressure = true;
      live.sendAudio(new ArrayBuffer(16));
    }
    if (failure === "finalize") {
      native.listener.onMessage(transcript());
      fixture.finalizeFailed = true;
    }
    assert.equal(await live.stop(), false);
    assert.equal(native.cancelled, true);
    fixture.backpressure = false;
  }
});

test("an account change during settings reads cannot start a connection with the previous account's key", async () => {
  providerSession();
  fixture.providerRead = () => {
    fixture.session = null;
  };
  const live = new SessionLiveTranscription("note-a", 16000, 1, () => {});
  await settle();
  assert.equal(fixture.native.length, 0);
  assert.equal(await live.stop(), false);
  assert.equal(fixture.listeners.size, 0);
});
