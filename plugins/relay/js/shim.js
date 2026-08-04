// Relay shim for browser contexts.
//
// Inside Tauri, __TAURI_INTERNALS__ is injected by the runtime and this
// script does nothing. In a regular browser the shim installs polyfills
// for every Tauri global and connects a WebSocket to the relay server
// (plugin-relay) so that invoke() calls are proxied to the real backend.

(function () {
  if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) return;

  // ---------------------------------------------------------------------------
  // Platform detection
  // ---------------------------------------------------------------------------

  function detectPlatform() {
    var ua = navigator.userAgent || "";
    var platform = "linux";
    if (/Mac|iPhone|iPad|iPod/.test(ua)) platform = "macos";
    else if (/Windows/.test(ua)) platform = "windows";
    else if (/Android/.test(ua)) platform = "android";
    return { platform: platform, isWindows: platform === "windows" };
  }

  function unregisterCallback(callbackId) {
    if (callbackId === null || callbackId === undefined) return;
    delete window["_" + callbackId];
  }

  // ---------------------------------------------------------------------------
  // WebSocket relay connection
  // ---------------------------------------------------------------------------

  function createRelayConnection(port) {
    var maxPendingInvokes = 64;
    var wsUrl = "ws://localhost:" + port + "/ws";
    var ws = null;
    var nextId = 0;
    var pending = {};
    var connected = false;
    var queue = [];

    // -- Outgoing ----------------------------------------------------------

    function removeQueuedInvoke(id) {
      queue = queue.filter(function (entry) {
        return entry.id !== id;
      });
    }

    function send(id, raw) {
      if (connected && ws && ws.readyState === 1) {
        ws.send(raw);
        return true;
      }

      if (queue.length >= maxPendingInvokes) return false;
      queue.push({ id: id, raw: raw });
      return true;
    }

    function invoke(command, args) {
      var id = ++nextId;
      var callbackId =
        command === "plugin:event|listen" && args ? args.handler : null;
      var raw;

      try {
        raw = JSON.stringify({ id: id, cmd: command, args: args || {} });
      } catch (error) {
        unregisterCallback(callbackId);
        return Promise.reject(error);
      }

      return new Promise(function (resolve, reject) {
        if (Object.keys(pending).length >= maxPendingInvokes) {
          unregisterCallback(callbackId);
          reject(new Error("relay invoke limit reached"));
          return;
        }

        pending[id] = {
          resolve: resolve,
          reject: reject,
          timeout: null,
          callbackId: callbackId,
        };
        var sent;
        try {
          sent = send(id, raw);
        } catch (error) {
          delete pending[id];
          unregisterCallback(callbackId);
          reject(error);
          return;
        }
        if (!sent) {
          delete pending[id];
          unregisterCallback(callbackId);
          reject(new Error("relay disconnected queue is full"));
          return;
        }

        pending[id].timeout = setTimeout(function () {
          var cb = pending[id];
          if (cb) {
            removeQueuedInvoke(id);
            delete pending[id];
            unregisterCallback(cb.callbackId);
            reject(new Error("relay invoke timed out"));
          }
        }, 30000);
      });
    }

    // -- Incoming ----------------------------------------------------------

    function handleInvokeResponse(msg) {
      var cb = pending[msg.id];
      if (!cb) return;
      delete pending[msg.id];
      clearTimeout(cb.timeout);

      if (msg.ok) {
        cb.resolve(msg.payload);
      } else {
        unregisterCallback(cb.callbackId);
        cb.reject(new Error(msg.payload || "invoke failed"));
      }
    }

    function handleEventPush(msg) {
      var handler = window["_" + msg.handler];
      if (handler) handler(msg.payload);
    }

    function onMessage(event) {
      try {
        var msg = JSON.parse(event.data);
        if (msg.type === "event") handleEventPush(msg);
        else handleInvokeResponse(msg);
      } catch {}
    }

    // -- Connection lifecycle -----------------------------------------------

    function flushQueue() {
      for (var i = 0; i < queue.length; i++) {
        var entry = queue[i];
        if (pending[entry.id]) ws.send(entry.raw);
      }
      queue = [];
    }

    function rejectAllPending() {
      queue = [];
      Object.keys(pending).forEach(function (id) {
        clearTimeout(pending[id].timeout);
        unregisterCallback(pending[id].callbackId);
        pending[id].reject(new Error("relay connection closed"));
        delete pending[id];
      });
    }

    function connect() {
      try {
        ws = new WebSocket(wsUrl);
      } catch {
        return;
      }

      ws.onopen = function () {
        connected = true;
        console.info("[relay] connected to " + wsUrl);
        flushQueue();
      };
      ws.onmessage = onMessage;
      ws.onclose = function (event) {
        connected = false;
        rejectAllPending();
        if (
          event &&
          event.code === 1013 &&
          window.location &&
          typeof window.location.reload === "function"
        ) {
          window.location.reload();
          return;
        }
        setTimeout(connect, 2000);
      };
      ws.onerror = function () {};
    }

    connect();

    return {
      invoke: invoke,
      nextId: function () {
        return ++nextId;
      },
    };
  }

  // ---------------------------------------------------------------------------
  // Install globals
  // ---------------------------------------------------------------------------

  var env = detectPlatform();
  var relayPort = Number("__RELAY_PORT__") || 1423;
  var relay = createRelayConnection(relayPort);

  // -- window.__TAURI_INTERNALS__ -------------------------------------------

  var metadata = {
    currentWindow: { label: "browser", kind: "WebviewWindow" },
    currentWebview: { label: "browser", windowLabel: "browser" },
    windows: [],
    webviews: [],
  };

  window.__TAURI_INTERNALS__ = {
    metadata: metadata,
    _metadata: metadata,
    plugins: {
      path: {
        sep: env.isWindows ? "\\" : "/",
        delimiter: env.isWindows ? ";" : ":",
      },
    },
    invoke: relay.invoke,
    unregisterCallback: unregisterCallback,
    transformCallback: function (callback, once) {
      var id = relay.nextId();
      window["_" + id] = function (response) {
        if (once) delete window["_" + id];
        if (callback) callback(response);
      };
      return id;
    },
    convertFileSrc: function (path) {
      return path;
    },
  };

  // -- window.__TAURI_EVENT_PLUGIN_INTERNALS__ ------------------------------

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: function (_event, eventId) {
      unregisterCallback(eventId);
    },
  };

  // -- window.__TAURI_OS_PLUGIN_INTERNALS__ ---------------------------------

  window.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: env.platform,
    os_type: env.platform,
    family: env.isWindows ? "windows" : "unix",
    version: navigator.userAgent,
    arch: "x86_64",
    eol: env.isWindows ? "\r\n" : "\n",
    exe_extension: env.isWindows ? "exe" : "",
  };
})();
