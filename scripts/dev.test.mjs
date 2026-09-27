import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const launcher = fileURLToPath(new URL("dev.mjs", import.meta.url));

function fixture(
  t,
  body = `console.log(JSON.stringify({ args: process.argv.slice(2), cwd: process.cwd(), env: process.env }));`,
) {
  const directory = mkdtempSync(join(tmpdir(), "anarlog-dev-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  writeFileSync(
    join(directory, "process-compose"),
    `#!${process.execPath}\n${body}\n`,
    { mode: 0o755 },
  );
  const env = {
    PATH: directory,
    PC_LOG_DIR: join(directory, "logs"),
  };
  return { directory, env };
}

for (const [target, port] of Object.entries({
  stack: "18080",
  desktop: "18081",
  web: "18082",
  api: "18083",
})) {
  test(`${target} selects the required stack from any working directory`, (t) => {
    const { directory, env } = fixture(t);
    const result = spawnSync(process.execPath, [launcher, target], {
      cwd: directory,
      env,
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    const invocation = JSON.parse(result.stdout.trim().split("\n").at(-1));
    assert.equal(invocation.cwd, root.replace(/\/$/, ""));
    assert.equal(invocation.env.PC_PORT_NUM, port);
    assert.equal(invocation.env.PC_DISABLE_TUI, "1");
    assert.equal(
      invocation.args.includes("process-compose.backend.yaml"),
      target === "stack" || target === "api",
    );
    assert.deepEqual(
      invocation.args.slice(invocation.args.indexOf("up")),
      target === "stack" ? ["up"] : ["up", target],
    );
    assert.ok(invocation.args.includes("--disable-dotenv"));
  });
}

test("preserves overrides and forwards flags without shell interpolation", (t) => {
  const { env } = fixture(t);
  env.PC_PORT_NUM = "19000";
  env.PC_DISABLE_TUI = "0";
  env.VITE_API_URL = "http://localhost:39001";
  const flags = ["--dry-run", "-f", "config with spaces and $(literal).yaml"];
  const result = spawnSync(
    process.execPath,
    [launcher, "web", "--", ...flags],
    { env, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  const invocation = JSON.parse(result.stdout.trim().split("\n").at(-1));
  assert.deepEqual(invocation.args.slice(-flags.length), flags);
  for (const key of [
    "PC_PORT_NUM",
    "PC_DISABLE_TUI",
    "PC_LOG_DIR",
    "VITE_API_URL",
  ]) {
    assert.equal(invocation.env[key], env[key]);
  }
});

test("returns the process-compose failure code", (t) => {
  const { env } = fixture(t, "process.exit(17);");
  const result = spawnSync(process.execPath, [launcher, "desktop"], {
    env,
    encoding: "utf8",
  });
  assert.equal(result.status, 17, result.stderr);
});

test("explains how to install a missing process-compose binary", (t) => {
  const { directory, env } = fixture(t);
  rmSync(join(directory, "process-compose"));
  const result = spawnSync(process.execPath, [launcher, "web"], {
    env,
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Install process-compose first/);
});

test("rejects unknown targets before starting processes", () => {
  const result = spawnSync(process.execPath, [launcher, "unknown"], {
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Usage:/);
});

test(
  "forwards shutdown signals to process-compose",
  { timeout: 10_000 },
  async (t) => {
    const { env } = fixture(
      t,
      `
    process.on("SIGTERM", () => process.exit(42));
    console.log("ready");
    setInterval(() => {}, 1000);
  `,
    );
    const child = spawn(process.execPath, [launcher, "desktop"], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    t.after(() => child.kill("SIGKILL"));
    const completion = once(child, "exit");
    for await (const chunk of child.stdout) {
      if (chunk.toString().includes("ready")) {
        child.kill("SIGTERM");
        break;
      }
    }
    const [code, signal] = await completion;
    assert.equal(signal, null);
    assert.equal(code, 42);
  },
);
