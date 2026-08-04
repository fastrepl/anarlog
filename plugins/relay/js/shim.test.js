import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const shim = readFileSync(new URL("./shim.js", import.meta.url), "utf8");

function loadShim() {
  const timers = [];
  const sockets = [];
  let reloads = 0;

  class FakeWebSocket {
    static OPEN = 1;

    constructor(url) {
      this.url = url;
      this.readyState = 0;
      this.sent = [];
      sockets.push(this);
    }

    send(message) {
      this.sent.push(message);
    }
  }

  const window = {
    location: {
      reload() {
        reloads += 1;
      },
    },
  };
  vm.runInNewContext(shim, {
    clearTimeout() {},
    console: { info() {} },
    navigator: { userAgent: "test" },
    setTimeout(callback, delay) {
      const timer = { callback, delay };
      timers.push(timer);
      return timer;
    },
    WebSocket: FakeWebSocket,
    window,
  });

  return {
    invoke: window.__TAURI_INTERNALS__.invoke,
    reloads: () => reloads,
    sockets,
    timers,
    window,
  };
}

test("timed-out disconnected invokes are not replayed", async () => {
  const relay = loadShim();
  const invocation = relay.invoke("test", {}).then(
    () => "resolved",
    (error) => error.message,
  );

  relay.timers.find((timer) => timer.delay === 30_000).callback();
  assert.equal(await invocation, "relay invoke timed out");

  const socket = relay.sockets[0];
  socket.readyState = 1;
  socket.onopen();
  assert.deepEqual(socket.sent, []);
});

test("disconnected invoke queue is bounded", async () => {
  const relay = loadShim();
  const queued = Array.from({ length: 64 }, (_, index) =>
    relay.invoke(`queued-${index}`, {}).catch((error) => error.message),
  );

  await assert.rejects(
    relay.invoke("overflow", {}),
    /relay invoke limit reached/,
  );

  relay.sockets[0].onclose({ code: 1006 });
  await Promise.all(queued);
});

test("connected stalled invokes are bounded", async () => {
  const relay = loadShim();
  const socket = relay.sockets[0];
  socket.readyState = 1;
  socket.onopen();
  const pending = Array.from({ length: 64 }, (_, index) =>
    relay.invoke(`stalled-${index}`, {}).catch((error) => error.message),
  );

  await assert.rejects(
    relay.invoke("overflow", {}),
    /relay invoke limit reached/,
  );
  assert.equal(socket.sent.length, 64);

  socket.onclose({ code: 1006 });
  await Promise.all(pending);
});

test("slow-client close reloads the browser to resubscribe", () => {
  const relay = loadShim();

  relay.sockets[0].onclose({ code: 1013 });

  assert.equal(relay.reloads(), 1);
});

test("unregistering an event listener releases its callback", () => {
  const relay = loadShim();
  const eventId = relay.window.__TAURI_INTERNALS__.transformCallback(() => {});

  assert.equal(typeof relay.window[`_${eventId}`], "function");
  relay.window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(
    "test-event",
    eventId,
  );
  assert.equal(relay.window[`_${eventId}`], undefined);
});

test("failed event listens release their transformed callback", async () => {
  const relay = loadShim();
  const callbackId = relay.window.__TAURI_INTERNALS__.transformCallback(
    () => {},
  );
  const socket = relay.sockets[0];
  socket.readyState = 1;
  socket.onopen();

  const listening = relay
    .invoke("plugin:event|listen", { handler: callbackId })
    .catch((error) => error.message);
  const invokeId = JSON.parse(socket.sent[0]).id;
  socket.onmessage({
    data: JSON.stringify({ id: invokeId, ok: false, payload: "listen failed" }),
  });

  assert.equal(await listening, "listen failed");
  assert.equal(relay.window[`_${callbackId}`], undefined);
});

test("timed-out event listens release their transformed callback", async () => {
  const relay = loadShim();
  const callbackId = relay.window.__TAURI_INTERNALS__.transformCallback(
    () => {},
  );
  const listening = relay
    .invoke("plugin:event|listen", { handler: callbackId })
    .catch((error) => error.message);

  relay.timers.find((timer) => timer.delay === 30_000).callback();

  assert.equal(await listening, "relay invoke timed out");
  assert.equal(relay.window[`_${callbackId}`], undefined);
});

test("unregisterCallback releases channel callbacks", () => {
  const relay = loadShim();
  const callbackId = relay.window.__TAURI_INTERNALS__.transformCallback(
    () => {},
  );

  relay.window.__TAURI_INTERNALS__.unregisterCallback(callbackId);

  assert.equal(relay.window[`_${callbackId}`], undefined);
});
