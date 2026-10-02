import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { buildRepository, writeRepository } from "./update-apt-repository.mjs";
import {
  readExpectedRepository,
  verifyRepository,
} from "./verify-apt-repository.mjs";

async function fixture(t) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "anarlog-live-apt-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const packages = Object.fromEntries(
    ["amd64", "arm64"].map((architecture) => [
      architecture,
      {
        control: `Package: anarlog\nVersion: 1.4.22\nArchitecture: ${architecture}\n`,
        hashes: {
          md5: "a".repeat(32),
          sha1: "a".repeat(40),
          sha256: "a".repeat(64),
        },
        size: 123,
      },
    ]),
  );
  await writeRepository(
    directory,
    buildRepository({
      version: "1.4.22",
      date: "2026-09-08T00:00:00Z",
      packages,
    }),
  );
  for (const file of [
    "anarlog-archive-keyring.asc",
    "dists/stable/InRelease",
    "dists/stable/Release.gpg",
  ]) {
    await writeFile(path.join(directory, file), `fixture ${file}`);
  }
  return directory;
}

test("verifies the key, signatures, and both architectures including compressed indexes", async (t) => {
  const files = await readExpectedRepository(await fixture(t), "1.4.22");
  const visited = [];
  await verifyRepository(files, async (url) => {
    assert.ok(url.startsWith("https://anarlog.so/apt/"));
    const file = url.slice("https://anarlog.so/apt/".length);
    visited.push(file);
    return new Response(files.get(file));
  });
  assert.equal(visited.length, 8);
  assert.ok(visited.includes("dists/stable/main/binary-arm64/Packages.gz"));
  assert.ok(visited.includes("dists/stable/main/binary-amd64/Packages.gz"));
});

test("rejects a checkout with the wrong release version before checking production", async (t) => {
  const directory = await fixture(t);
  await assert.rejects(
    readExpectedRepository(directory, "1.4.21"),
    /Expected APT metadata for 1.4.21/,
  );
  await assert.rejects(
    readExpectedRepository(directory, undefined),
    /must be a stable version/,
  );
});

test("rejects each stale or corrupted live file, even when the Release version matches", async (t) => {
  const files = await readExpectedRepository(await fixture(t), "1.4.22");
  for (const staleFile of files.keys()) {
    await assert.rejects(
      verifyRepository(files, async (url) => {
        const file = url.slice("https://anarlog.so/apt/".length);
        return new Response(file === staleFile ? "stale" : files.get(file));
      }),
      /Live APT file differs/,
    );
  }
});

test("rejects unsuccessful responses and network failures", async (t) => {
  const files = await readExpectedRepository(await fixture(t), "1.4.22");
  await assert.rejects(
    verifyRepository(
      files,
      async () => new Response("unavailable", { status: 503 }),
    ),
    /failed with 503/,
  );
  await assert.rejects(
    verifyRepository(files, async () => {
      throw new Error("network unavailable");
    }),
    /network unavailable/,
  );
});

test("the merge step waits for checks and refuses failed checks or a changed PR head", async (t) => {
  const publish = await readFile(
    ".github/workflows/desktop_publish.yaml",
    "utf8",
  );
  const step = publish
    .split("      - id: merge\n")[1]
    .split("\n  linux-apt-deploy:\n")[0];
  const script = step.split("        run: |\n")[1].replace(/^          /gm, "");
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "anarlog-package-merge-"),
  );
  t.after(() => rm(directory, { recursive: true, force: true }));
  const head = "a".repeat(40);
  const merged = "b".repeat(40);
  for (const scenario of [
    "success",
    "failed-checks",
    "changed-head",
    "unmerged",
  ]) {
    const output = path.join(directory, `${scenario}.output`);
    await writeFile(output, "");
    const mocks = `
      waited=false
      checked=false
      sleep() { waited=true; }
      gh() {
        if [[ "$*" == *statusCheckRollup* ]]; then
          if [[ "$waited" == true ]]; then
            echo '["ci", "fmt", "lint", "zizmor"]'
          else
            echo '[]'
          fi
        elif [[ "$1 $2" == 'pr checks' ]]; then
          [[ "$waited" == true ]] || return 90
          [[ "$*" == *'--watch --fail-fast'* ]] || return 91
          [[ "$SCENARIO" != failed-checks ]] || return 1
          checked=true
        elif [[ "$1 $2" == 'pr merge' ]]; then
          [[ "$checked" == true ]] || return 92
          [[ "$*" == "pr merge 123 --squash --match-head-commit $PR_HEAD_SHA" ]] || return 93
          [[ "$SCENARIO" != changed-head ]] || return 1
          echo merge-requested >> "$GITHUB_OUTPUT"
        elif [[ "$*" == *mergeCommit* ]]; then
          if [[ "$SCENARIO" != unmerged ]]; then echo '${merged}'; fi
        else
          return 94
        fi
      }
    `;
    const result = spawnSync(
      "bash",
      ["--noprofile", "--norc", "-eo", "pipefail", "-c", `${mocks}\n${script}`],
      {
        env: {
          ...process.env,
          PR_NUMBER: "123",
          PR_HEAD_SHA: head,
          GITHUB_OUTPUT: output,
          SCENARIO: scenario,
        },
        encoding: "utf8",
        timeout: 5000,
      },
    );
    assert.ifError(result.error);
    const written = await readFile(output, "utf8");
    if (scenario === "success") {
      assert.equal(result.status, 0, result.stderr);
      assert.match(written, new RegExp(`sha=${merged}`));
    } else {
      assert.notEqual(result.status, 0);
      assert.doesNotMatch(written, /sha=/);
      if (scenario !== "unmerged")
        assert.doesNotMatch(written, /merge-requested/);
    }
  }
});
