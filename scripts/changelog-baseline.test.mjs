import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { changelogBaseline } from "./changelog-baseline.mjs";

test("backfilled notes compare the requested release with its earlier channel predecessor", (t) => {
  const cwd = mkdtempSync(join(tmpdir(), "anarlog-changelog-baseline-"));
  t.after(() => rmSync(cwd, { recursive: true, force: true }));
  const git = (...args) =>
    execFileSync("git", args, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  git("init");
  git("config", "user.name", "Test");
  git("config", "user.email", "test@example.com");
  const directory = join(cwd, "packages/changelog/content/mobile");
  mkdirSync(directory, { recursive: true });
  const sources = [];
  for (const [version, channel] of [
    ["1.4.0", "stable"],
    ["1.4.1", "beta"],
    ["1.4.2", "stable"],
    ["1.4.3", "beta"],
  ]) {
    git("commit", "--allow-empty", "-m", version);
    const sourceSha = git("rev-parse", "HEAD");
    sources.push(sourceSha);
    git("tag", `desktop_v${version}`);
    writeFileSync(
      join(directory, `${version}.json`),
      JSON.stringify({
        version,
        sourceSha,
        channel,
        availability: [{ publishedAt: "2026-01-01T00:00:00Z" }],
      }),
    );
  }
  assert.deepEqual(changelogBaseline("desktop", "stable", "1.4.1", cwd), {
    prev: "desktop_v1.4.0",
    current: "desktop_v1.4.1",
  });
  assert.deepEqual(changelogBaseline("mobile", "beta", "1.4.1", cwd), {
    prev: sources[0],
    current: sources[1],
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.2", cwd), {
    prev: sources[0],
    current: sources[2],
  });
  assert.deepEqual(changelogBaseline("mobile", "beta", "1.4.4", cwd), {
    prev: sources[3],
    current: "HEAD",
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.4", cwd), {
    prev: sources[2],
    current: "HEAD",
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.0", cwd), {
    prev: sources[0],
    current: sources[0],
  });
  assert.throws(
    () => changelogBaseline("desktop", "beta", "1.4.1", cwd),
    /Unsupported/,
  );
  git("tag", "-f", "desktop_v1.4.0", sources[3]);
  assert.throws(() => changelogBaseline("desktop", "stable", "1.4.1", cwd));
});
