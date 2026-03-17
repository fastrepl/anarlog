#!/usr/bin/env node

const { spawn } = require("node:child_process");
const { join } = require("node:path");

const bin = join(__dirname, "char");
const child = spawn(bin, process.argv.slice(2), { stdio: "inherit" });

child.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 1);
  }
});
