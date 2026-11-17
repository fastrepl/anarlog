// Serves the shipping desktop frontend in a plain browser with Tauri IPC mocked, backed by
// the library the native screenshot harness seeded, so both apps render the same data.
//
// PARITY_DB=/tmp/screens/library.sqlite node apps/desktop-gpui/parity/serve.mjs
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

import { commandDefaults } from "./defaults.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const desktop = path.resolve(here, "../../desktop");
const port = Number(process.env.PARITY_PORT ?? 1499);
const database = process.env.PARITY_DB;
if (!database)
  throw new Error("Set PARITY_DB to a disposable copy of a library.sqlite");

const db = new DatabaseSync(database);
const MOCK_ID = "virtual:parity-ipc";
const DEFAULTS_ID = "virtual:parity-defaults";
const defaults = JSON.stringify(
  commandDefaults(path.resolve(here, "../../..")),
);

const bind = (params = []) =>
  params.map((value) => {
    if (typeof value === "boolean") return value ? 1 : 0;
    if (value !== null && typeof value === "object")
      return JSON.stringify(value);
    return value ?? null;
  });

const rows = (sql, params, arrays) => {
  const statement = db.prepare(sql);
  statement.setReturnArrays(arrays);
  return statement.all(...bind(params));
};

// Mirrors plugins/db's execute, execute_proxy and execute_transaction result shapes.
const commands = {
  execute: ({ sql, params }) => rows(sql, params, false),
  subscribe: ({ sql, params }) => rows(sql, params, false),
  execute_proxy: ({ sql, params, method }) => {
    if (method === "run") {
      db.prepare(sql).run(...bind(params));
      return { rows: [] };
    }
    const result = rows(sql, params, true);
    return { rows: method === "get" ? (result[0] ?? []) : result };
  },
  execute_transaction: ({ statements }) => {
    db.exec("BEGIN");
    try {
      const changes = statements.map(({ sql, params }) =>
        Number(db.prepare(sql).run(...bind(params)).changes),
      );
      db.exec("COMMIT");
      return changes;
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  },
};

const parity = {
  name: "anarlog-parity",
  resolveId: (id) =>
    [MOCK_ID, DEFAULTS_ID].includes(id) ? `\0${id}` : undefined,
  load(id) {
    if (id === `\0${MOCK_ID}`)
      return readFileSync(path.join(here, "mock-ipc.js"), "utf8");
    if (id === `\0${DEFAULTS_ID}`) return `export default ${defaults};`;
  },
  transformIndexHtml: () => [
    // Runs before relay-shim.js so it sees Tauri internals and stays inactive.
    {
      tag: "script",
      children:
        "window.__TAURI_INTERNALS__ ??= { invoke: () => Promise.resolve(null) };",
      injectTo: "head-prepend",
    },
    {
      tag: "script",
      attrs: { type: "module", src: `/@id/${MOCK_ID}` },
      injectTo: "head-prepend",
    },
  ],
  configureServer(server) {
    server.middlewares.use("/__parity/db", (request, response) => {
      let body = "";
      request.on("data", (chunk) => (body += chunk));
      request.on("end", () => {
        const { command, args } = JSON.parse(body);
        response.setHeader("content-type", "application/json");
        try {
          const handler = commands[command];
          if (!handler) throw new Error(`unmocked database command ${command}`);
          response.end(JSON.stringify({ value: handler(args) }));
        } catch (error) {
          response.statusCode = 500;
          response.end(
            JSON.stringify({ error: String(error?.message ?? error) }),
          );
        }
      });
    });
  },
};

// Lingui and other tools in the desktop config resolve from the working directory.
process.chdir(desktop);
const require = createRequire(path.join(desktop, "package.json"));
const { createServer } = await import(require.resolve("vite"));
const server = await createServer({
  configFile: path.join(desktop, "vite.config.ts"),
  root: desktop,
  // Keep the dependency cache away from the desktop app's own dev server.
  cacheDir: path.join(os.tmpdir(), `anarlog-parity-vite-${port}`),
  plugins: [parity],
  server: { port, strictPort: true, host: "127.0.0.1" },
});
await server.listen();
console.log(`parity frontend on http://127.0.0.1:${port}/app/main`);
