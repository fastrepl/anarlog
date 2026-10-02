import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(
  new URL("../.github/workflows/pr-description.yml", import.meta.url),
  "utf8",
);
const script = workflow.match(/node <<'NODE'\n([\s\S]*?)\n\s+NODE/)?.[1];
assert.ok(script, "PR description workflow must contain its validation script");

function validate(demo, overrides = {}) {
  return spawnSync(process.execPath, ["-e", script], {
    encoding: "utf8",
    env: {
      ...process.env,
      PR_AUTHOR_ASSOCIATION: "NONE",
      PR_AUTHOR_TYPE: "User",
      PR_TITLE: "Prevent duplicate notes after reconnect",
      PR_BODY: `## Summary\n\n**Intent:** Prevent duplicate notes when the connection recovers.\n\n## Demo\n\n${demo}\n\n## Verification\n\nManually checked reconnecting.`,
      ...overrides,
    },
  });
}

test("accepts a visible demo link or an N/A reason", () => {
  for (const demo of [
    "https://github.com/user-attachments/assets/demo",
    "[Recording](https://vimeo.com/12345)",
    "N/A docs only",
    "N/A: docs",
    "N/A — CI only",
  ]) {
    const result = validate(demo);
    assert.equal(result.status, 0, `${demo}\n${result.stderr}`);
  }
});

test("rejects missing, hidden, or reasonless demo evidence", () => {
  for (const demo of [
    "",
    "demo.mp4",
    "```text\nhttps://example.com/demo.gif\n```",
    "    https://example.com/demo.gif",
    "<!-- https://example.com/demo.gif -->",
    "N/A",
    "N/A —",
    "N/A\nThis reason belongs on the same line.",
    "No recording.\n\n## Other\nhttps://example.com/demo.gif",
  ]) {
    const result = validate(demo);
    assert.equal(result.status, 1, JSON.stringify(demo));
    assert.match(result.stderr, /## Demo/);
  }
});

test("rejects a missing, overly verbose, or headingless Intent/Demo summary", () => {
  for (const body of [
    "## Summary\n\n## Demo\nN/A docs",
    `## Summary\n\n**Intent:** ${"a".repeat(401)}\n\n## Demo\nN/A docs`,
    "## Summary\n\n**Intent:** Prevent duplicate notes when the connection recovers.",
  ]) {
    assert.equal(validate("", { PR_BODY: body }).status, 1, body);
  }
});

test("exempts maintainers and bots", () => {
  for (const overrides of [
    { PR_AUTHOR_ASSOCIATION: "OWNER" },
    { PR_AUTHOR_TYPE: "Bot" },
  ]) {
    const result = validate("", { PR_BODY: "", PR_TITLE: "", ...overrides });
    assert.equal(result.status, 0, result.stderr);
  }
});
