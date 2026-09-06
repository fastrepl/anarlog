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
});
const mocks = {
  "@/auth/client": `const f = globalThis.liveAccessFixture; export const supabase = { auth: {
    getSession: async () => ({ data: { session: f.session } }),
    onAuthStateChange: listener => { f.listeners.add(listener); return { data: { subscription: { unsubscribe: () => f.listeners.delete(listener) } } }; }
  } };`,
  "@/db": `export async function execute() { return []; } export async function executeTransaction() { return []; }`,
  "@/settings/providers": `export async function readProviderConfig() { return { provider: 'anarlog' }; }`,
  "@/settings/preferences": `export async function readPreferences() { return {}; }`,
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

const { HostedLiveTranscription } = await import("./live-transcription.ts");

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
    const live = new HostedLiveTranscription("note-a", 16000, 1, (update) =>
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
  const live = new HostedLiveTranscription("note-a", 16000, 1, (update) =>
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
  const live = new HostedLiveTranscription("note-a", 16000, 1, () => {});
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
