import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { notarizeDmg, runCommand } from "./notarize-dmg.mjs";

test("process deadline bounds waits without cutting off a large upload", () => {
  for (const command of ["submit", "wait"]) {
    runCommand(["notarytool", command], (executable, args, options) => {
      assert.equal(executable, "xcrun");
      assert.equal(args[1], command);
      assert.equal(options.timeout, command === "wait" ? 660_000 : undefined);
      assert.equal(options.killSignal, "SIGKILL");
    });
  }
});

const id = "bb9d8b7c-582d-4111-a1e8-2d67289f0f2d";
const reply = (status, exit = 0) => ({
  status: exit,
  stdout: JSON.stringify({ id, status }),
  stderr: "",
});
const offline = {
  status: 1,
  stdout: "",
  stderr: "Error Domain=NSURLErrorDomain Code=-1009 Internet appears offline",
};

function fixture(results) {
  const calls = [];
  const delays = [];
  return {
    calls,
    delays,
    options: {
      dmgPath: "/tmp/test package.dmg",
      env: {
        APPLE_ID: "test",
        APPLE_PASSWORD: "secret",
        APPLE_TEAM_ID: "team",
      },
      run: (args) => {
        calls.push(args);
        assert.ok(results.length);
        return results.shift();
      },
      sleep: async (ms) => {
        delays.push(ms);
      },
      log: () => {},
    },
  };
}

test("submits once and recovers interrupted waits using the same ID", async () => {
  const f = fixture([reply("Uploaded"), offline, reply("Accepted")]);
  assert.equal(await notarizeDmg(f.options), id);
  assert.deepEqual(
    f.calls.map((args) => args.slice(0, 3)),
    [
      ["notarytool", "submit", "/tmp/test package.dmg"],
      ["notarytool", "wait", id],
      ["notarytool", "wait", id],
    ],
  );
  assert.ok(f.calls[0].includes("--no-wait"));
  assert.ok(f.calls[1].includes("10m"));
  assert.deepEqual(f.delays, [30_000]);
});

test("bounds persistent transport errors to four waits", async () => {
  const f = fixture([reply("Uploaded"), offline, offline, offline, offline]);
  await assert.rejects(notarizeDmg(f.options), /attempt 4\/4/);
  assert.equal(f.calls.length, 5);
  assert.deepEqual(f.delays, [30_000, 60_000, 90_000]);
});

for (const result of [
  reply("Invalid", 1),
  reply("Rejected", 1),
  reply("In Progress"),
  { status: 1, stderr: "HTTP 401 Unauthorized" },
  { status: 0, stdout: "not JSON" },
  { status: null, signal: "SIGKILL" },
]) {
  test(`fails closed without retry for ${JSON.stringify(result)}`, async () => {
    const f = fixture([reply("Uploaded"), result]);
    await assert.rejects(notarizeDmg(f.options));
    assert.equal(f.calls.length, 2);
    assert.deepEqual(f.delays, []);
  });
}

test("never resubmits an ambiguous or malformed upload response", async () => {
  for (const result of [
    offline,
    { status: 0, stdout: "{}" },
    { status: 0, stdout: '{"id":"bad"}' },
  ]) {
    const f = fixture([result]);
    await assert.rejects(notarizeDmg(f.options), /submission history/);
    assert.equal(f.calls.length, 1);
  }
});

test("rejects mismatched submission identity", async () => {
  const f = fixture([
    reply("Uploaded"),
    { status: 0, stdout: JSON.stringify({ id: "other", status: "Accepted" }) },
  ]);
  await assert.rejects(notarizeDmg(f.options), /different/);
});

test("retries known temporary HTTP and timeout failures on the same submission", async () => {
  for (const stderr of [
    "HTTPError(statusCode: Optional(503))",
    "Timeout reached",
    "NSURLErrorDomain Code=-1005",
  ]) {
    const f = fixture([
      reply("Uploaded"),
      { status: 1, stderr },
      reply("Accepted"),
    ]);
    assert.equal(await notarizeDmg(f.options), id);
    assert.equal(f.calls.length, 3);
  }
});

test("does not retry an explicit rejection even alongside a network diagnostic", async () => {
  const f = fixture([
    reply("Uploaded"),
    { ...reply("Invalid", 1), stderr: offline.stderr },
  ]);
  await assert.rejects(notarizeDmg(f.options), /Invalid/);
  assert.equal(f.calls.length, 2);
});

test("an accepted response must identify the submission", async () => {
  const f = fixture([
    reply("Uploaded"),
    { status: 0, stdout: '{"status":"Accepted"}' },
  ]);
  await assert.rejects(notarizeDmg(f.options), /different/);
});

test("action gates stapling and Gatekeeper verification behind accepted notarization", () => {
  const action = readFileSync(
    new URL(
      "../.github/actions/macos_notarize_dmg/action.yaml",
      import.meta.url,
    ),
    "utf8",
  );
  assert.match(action, /set -euo pipefail/);
  const steps = [
    'node scripts/notarize-dmg.mjs "$DMG_PATH"',
    'xcrun stapler staple "$DMG_PATH"',
    'xcrun stapler validate "$DMG_PATH"',
    "spctl -a -t open -vvv",
  ];
  let previous = -1;
  for (const step of steps) {
    const position = action.indexOf(step);
    assert.ok(position > previous, step);
    previous = position;
  }
  assert.doesNotMatch(action.split("run: |")[1], /\$\{\{ inputs\./);
});
