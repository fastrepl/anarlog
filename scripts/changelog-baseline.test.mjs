import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { changelogBaseline } from "./changelog-baseline.mjs";

test("release notes include post-beta fixes, honor explicit promotion sources and identify initial releases", (t) => {
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
  git("commit", "--allow-empty", "-m", "Unshipped work after the beta build");
  assert.notEqual(git("rev-parse", "HEAD"), sources[3]);
  assert.deepEqual(changelogBaseline("desktop", "stable", "1.4.1", cwd), {
    prev: "desktop_v1.4.0",
    current: "desktop_v1.4.1",
    initial: false,
  });
  assert.deepEqual(changelogBaseline("mobile", "beta", "1.4.1", cwd), {
    prev: sources[0],
    current: sources[1],
    initial: false,
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.2", cwd), {
    prev: sources[0],
    current: sources[2],
    initial: false,
  });
  assert.deepEqual(changelogBaseline("mobile", "beta", "1.4.4", cwd), {
    prev: sources[3],
    current: "HEAD",
    initial: false,
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.4", cwd), {
    prev: sources[2],
    current: "HEAD",
    initial: false,
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.0", cwd), {
    prev: sources[0],
    current: sources[0],
    initial: true,
  });
  assert.deepEqual(changelogBaseline("mobile", "stable", "1.4.3", cwd), {
    prev: sources[2],
    current: "HEAD",
    initial: false,
  });
  assert.deepEqual(
    changelogBaseline("mobile", "stable", "1.4.3", cwd, sources[3]),
    {
      prev: sources[2],
      current: sources[3],
      initial: false,
    },
  );
  assert.throws(
    () => changelogBaseline("mobile", "stable", "1.4.3", cwd, "HEAD"),
    /full commit SHA/,
  );
  assert.throws(
    () => changelogBaseline("desktop", "beta", "1.4.1", cwd),
    /Unsupported/,
  );
  git("tag", "-f", "desktop_v1.4.0", sources[3]);
  assert.throws(() => changelogBaseline("desktop", "stable", "1.4.1", cwd));
});
