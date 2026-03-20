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

  // ---------------------------------------------------------------------------
  // WebSocket relay connection
  // ---------------------------------------------------------------------------

  function createRelayConnection(port) {
    var wsUrl = "ws://localhost:" + port + "/ws";
    var ws = null;
    var nextId = 0;
    var pending = {};
    var connected = false;
    var queue = [];

    // -- Outgoing ----------------------------------------------------------

    function send(raw) {
      if (connected && ws && ws.readyState === 1) {
        ws.send(raw);
      } else {
        queue.push(raw);
      }
    }

    function invoke(command, args) {
      var id = ++nextId;
      var raw = JSON.stringify({ id: id, cmd: command, args: args || {} });

      return new Promise(function (resolve, reject) {
        pending[id] = { resolve: resolve, reject: reject };
        send(raw);

        setTimeout(function () {
          if (pending[id]) {
            delete pending[id];
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

      if (msg.ok) {
        cb.resolve(msg.payload);
      } else {
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
      } catch (_) {}
    }

    // -- Connection lifecycle -----------------------------------------------

    function flushQueue() {
      for (var i = 0; i < queue.length; i++) ws.send(queue[i]);
      queue = [];
    }

    function rejectAllPending() {
      Object.keys(pending).forEach(function (id) {
        pending[id].reject(new Error("relay connection closed"));
        delete pending[id];
      });
    }

    function connect() {
      try {
        ws = new WebSocket(wsUrl);
      } catch (_) {
        return;
      }

      ws.onopen = function () {
        connected = true;
        console.info("[relay] connected to " + wsUrl);
        flushQueue();
      };
      ws.onmessage = onMessage;
      ws.onclose = function () {
        connected = false;
        rejectAllPending();
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
  var relay = createRelayConnection(1423);

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
    unregisterListener: function () {},
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
