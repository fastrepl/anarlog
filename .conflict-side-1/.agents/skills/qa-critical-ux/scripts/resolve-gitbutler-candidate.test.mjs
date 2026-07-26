import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const resolver = fileURLToPath(
  new URL("./resolve-gitbutler-candidate.mjs", import.meta.url),
);
const candidate = "0123456789abcdef0123456789abcdef01234567";
const otherCandidate = "89abcdef0123456789abcdef0123456789abcdef";

function resolve(status, explicitCandidate) {
  return spawnSync(
    process.execPath,
    [resolver, ...(explicitCandidate ? [explicitCandidate] : [])],
    {
      encoding: "utf8",
      input: JSON.stringify(status),
    },
  );
}

test("derives the top branch tip from one applied stack", () => {
  const result = resolve({
    stacks: [
      {
        branches: [
          { commits: [{ commitId: candidate }] },
          { commits: [{ commitId: otherCandidate }] },
        ],
      },
    ],
  });

  assert.equal(result.status, 0);
  assert.equal(result.stdout, candidate);
});

test("fails closed when multiple stacks are applied", () => {
  const result = resolve({
    stacks: [
      { branches: [{ commits: [{ commitId: candidate }] }] },
      { branches: [{ commits: [{ commitId: otherCandidate }] }] },
    ],
  });

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /Expected exactly one applied GitButler stack, found 2/,
  );
});

test("prefers an explicit candidate over workspace derivation", () => {
  const result = resolve({ stacks: [] }, otherCandidate.toUpperCase());

  assert.equal(result.status, 0);
  assert.equal(result.stdout, otherCandidate);
});
