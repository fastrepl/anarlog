import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const [target, ...args] = process.argv.slice(2);
const ports = { stack: "18080", desktop: "18081", web: "18082", api: "18083" };

if (!Object.hasOwn(ports, target)) {
  console.error(
    "Usage: node scripts/dev.mjs <stack|desktop|web|api> [process-compose flags]",
  );
  process.exit(1);
}

const root = fileURLToPath(new URL("../", import.meta.url));
process.chdir(root);

const env = {
  ...process.env,
  PC_PORT_NUM: process.env.PC_PORT_NUM || ports[target],
  PC_LOG_DIR: process.env.PC_LOG_DIR || `.process-compose/${target}`,
};
if (
  !process.stdout.isTTY ||
  !process.stdin.isTTY ||
  process.env.TERM === "dumb"
) {
  env.PC_DISABLE_TUI ??= "1";
}
mkdirSync(env.PC_LOG_DIR, { recursive: true });
console.log(`Logs: ${env.PC_LOG_DIR}`);

const composeArgs = ["--disable-dotenv", "-f", "process-compose.yaml"];
if (target === "stack" || target === "api") {
  composeArgs.push("-f", "process-compose.backend.yaml");
}
composeArgs.push("up");
if (target !== "stack") {
  composeArgs.push(target);
}
if (args[0] === "--") {
  args.shift();
}
composeArgs.push(...args);

const child = spawn("process-compose", composeArgs, {
  cwd: root,
  env,
  stdio: "inherit",
});
const signalExitCodes = { SIGINT: 130, SIGTERM: 143 };
for (const signal of Object.keys(signalExitCodes)) {
  process.on(signal, () => child.kill(signal));
}
child.on("error", (error) => {
  console.error(
    error.code === "ENOENT"
      ? "Install process-compose first (macOS: brew install process-compose). See CONTRIBUTING.md."
      : error.message,
  );
  process.exit(1);
});
child.on("exit", (code, signal) => {
  process.exit(code ?? signalExitCodes[signal] ?? 1);
});
