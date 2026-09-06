import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

function runHook(t, platform, { ndk = true } = {}) {
  const root = mkdtempSync(join(tmpdir(), "anarlog-eas-hook-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const scripts = join(root, "apps/mobile/scripts");
  const bin = join(root, "cargo/bin");
  const log = join(root, "commands.log");
  mkdirSync(scripts, { recursive: true });
  mkdirSync(bin, { recursive: true });
  mkdirSync(join(root, "apps/mobile/ios"), { recursive: true });
  if (ndk) {
    const ndkPath = join(root, "sdk/ndk/27.1.12297006");
    mkdirSync(ndkPath, { recursive: true });
    writeFileSync(
      join(ndkPath, "source.properties"),
      "Pkg.Revision = 27.1.12297006\n",
    );
  }
  copyFileSync(
    new URL("./eas-build-post-install.sh", import.meta.url),
    join(scripts, "hook.sh"),
  );
  writeFileSync(log, "");
  for (const command of ["rustup", "cargo", "pod"]) {
    writeFileSync(
      join(bin, command),
      `#!/bin/bash\nprintf '%s|%s|%s|%s\\n' '${command}' "$*" "$PWD" "\${ANDROID_NDK_HOME:-}" >> "$ANARLOG_TEST_COMMAND_LOG"\n`,
      { mode: 0o755 },
    );
  }
  const result = spawnSync("bash", [join(scripts, "hook.sh")], {
    encoding: "utf8",
    env: {
      PATH: "/usr/bin:/bin",
      CARGO_HOME: join(root, "cargo"),
      EAS_BUILD_PLATFORM: platform,
      ANDROID_HOME: join(root, "sdk"),
      ANARLOG_TEST_COMMAND_LOG: log,
    },
  });
  return {
    result,
    root,
    commands: readFileSync(log, "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => line.split("|")),
  };
}

test("EAS Android generates every native bridge ABI before packaging", (t) => {
  const { result, root, commands } = runHook(t, "android");
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(
    commands.map(([command, args]) => [command, args]),
    [
      [
        "rustup",
        "target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android",
      ],
      ["cargo", "install cargo-ndk --version 4.1.2 --locked"],
      ["cargo", "xtask mobile-bridge android"],
    ],
  );
  assert.equal(commands.at(-1)[2], root);
  assert.equal(commands.at(-1)[3], join(root, "sdk/ndk/27.1.12297006"));
});

test("EAS iOS refreshes pods after generating frameworks", (t) => {
  const { result, root, commands } = runHook(t, "ios");
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(
    commands.map(([command, args]) => [command, args]),
    [
      [
        "rustup",
        "target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios",
      ],
      ["cargo", "xtask mobile-bridge ios"],
      ["pod", "install"],
    ],
  );
  assert.equal(commands.at(-1)[2], join(root, "apps/mobile/ios"));
});

test("EAS Android fails before compilation when its NDK is missing", (t) => {
  const { result, commands } = runHook(t, "android", { ndk: false });
  assert.notEqual(result.status, 0);
  assert.equal(
    commands.some(([command]) => command === "cargo"),
    false,
  );
});

test("ordinary installs do not invoke native build tools", (t) => {
  const { result, commands } = runHook(t, "");
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(commands, []);
});
