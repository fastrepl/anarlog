import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmod, mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const launcher = fileURLToPath(
  new URL("./launch-native-dev-qa.sh", import.meta.url),
);

async function fixture() {
  const directory = await mkdtemp(path.join(tmpdir(), "anarlog-native-qa-"));
  const appBundle = path.join(directory, "Anarlog Dev.app");
  const fakeOpen = path.join(directory, "open");

  await mkdir(appBundle);
  await writeFile(fakeOpen, "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\"\n");
  await chmod(fakeOpen, 0o755);

  return {
    appBundle,
    directory,
    launch(environment = {}) {
      const childEnvironment = {
        ...process.env,
        ANARLOG_QA_OPEN_EXECUTABLE: fakeOpen,
        ...environment,
      };
      if (!("ONBOARDING" in environment)) {
        delete childEnvironment.ONBOARDING;
      }

      return spawnSync(launcher, [appBundle], {
        encoding: "utf8",
        env: childEnvironment,
      });
    },
  };
}

test("launches the app bundle through LaunchServices with QA probes", async (t) => {
  const harness = await fixture();
  t.after(() => rm(harness.directory, { force: true, recursive: true }));

  const result = harness.launch();

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.stdout.trim().split("\n"), [
    "-W",
    "--env",
    "AUDIO_SYNC_PROBE=1",
    "--env",
    "LISTENER_DEBUG=1",
    "--env",
    "NO_AEC=",
    harness.appBundle,
  ]);
});

test("forwards the onboarding reset through LaunchServices", async (t) => {
  const harness = await fixture();
  t.after(() => rm(harness.directory, { force: true, recursive: true }));

  const result = harness.launch({ ONBOARDING: "1" });

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.stdout.trim().split("\n").slice(-3), [
    "--env",
    "ONBOARDING=1",
    harness.appBundle,
  ]);
});
