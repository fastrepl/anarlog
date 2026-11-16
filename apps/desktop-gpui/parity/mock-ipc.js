// Loaded before the desktop entry point by serve.mjs. Answers Tauri IPC in a plain browser:
// database commands go to the seeded library, generated plugin commands get a neutral value
// of their declared type, and anything else is logged once as "[parity] unmocked <command>".
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import defaults from "virtual:parity-defaults";

window.__TAURI_OS_PLUGIN_INTERNALS__ = {
  platform: "macos",
  os_type: "macos",
  family: "unix",
  version: "27.0",
  arch: "aarch64",
  exe_extension: "",
  eol: "\n",
};
mockWindows("main");

// `?tabs=<pinned tabs JSON>` opens screens through the app's own pinned-tab restore.
const pinnedTabs = new URLSearchParams(window.location.search).get("tabs");
const unmocked = new Set();
let subscriptions = 0;

async function database(command, args) {
  const response = await fetch("/__parity/db", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ command, args: { ...args, onEvent: undefined } }),
  });
  const result = await response.json();
  if (result.error) throw result.error;
  return result.value;
}

// Tauri core commands have no generated bindings.
const core = {
  "plugin:app|identifier": "com.hyprnote.dev",
  "plugin:app|name": "Anarlog",
  "plugin:app|version": "0.0.0",
  "plugin:window|theme": "light",
  "plugin:window|is_fullscreen": false,
  "plugin:window|is_maximized": false,
  "plugin:window|is_visible": true,
  "plugin:window|is_focused": true,
  "plugin:db|get_startup_status": {
    phase: "ready",
    migrationCurrent: null,
    migrationTotal: null,
  },
};

const handlers = {
  get_pinned_tabs: () => pinnedTabs,
  "plugin:db|execute": (args) => database("execute", args),
  "plugin:db|execute_proxy": (args) => database("execute_proxy", args),
  "plugin:db|execute_transaction": (args) =>
    database("execute_transaction", args),
  "plugin:db|subscribe": async (args) => {
    const rows = await database("subscribe", args);
    args.onEvent.onmessage({ event: "result", data: rows });
    subscriptions += 1;
    return {
      id: `parity-${subscriptions}`,
      analysis: { kind: "reactive", data: {} },
    };
  },
};

mockIPC(
  (command, args) => {
    const handler = handlers[command];
    if (handler) return handler(args);
    if (command in core) return structuredClone(core[command]);
    if (command in defaults) return structuredClone(defaults[command]);
    if (!unmocked.has(command)) {
      unmocked.add(command);
      console.warn(`[parity] unmocked ${command}`);
    }
    return null;
  },
  { shouldMockEvents: true },
);
