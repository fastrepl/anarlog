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

for (const demo of [
  "https://github.com/user-attachments/assets/demo",
  "[Recording](https://vimeo.com/12345)",
  "https://drive.google.com/file/d/demo/view",
  "https://www.dropbox.com/s/demo/recording",
  "https://www.dailymotion.com/video/demo",
  "https://cdn.example.com/signed-demo?token=abc",
  "https://example.com/demo.gif",
  "N/A docs only",
  "N/A: docs",
  "N/A — CI only",
]) {
  test(`accepts visible demo: ${demo}`, () => {
    const result = validate(demo);
    assert.equal(result.status, 0, result.stderr);
  });
}

for (const demo of [
  "",
  "The demo filename is example.gif",
  "demo.mp4",
  "```text\nhttps://example.com/demo.gif\n```",
  "~~~\nhttps://example.com/demo.gif\n~~~",
  "    https://example.com/demo.gif",
  "<!-- https://example.com/demo.gif -->",
  "N/A",
  "N/A —",
  "N/A\nThis reason belongs on the same line.",
  "N/A\n\n```\nThis is only filler in a code block.\n```",
  "No recording.\n\n## Other\nhttps://example.com/demo.gif",
]) {
  test(`rejects missing demo evidence: ${JSON.stringify(demo)}`, () => {
    const result = validate(demo);
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, /## Demo/);
  });
}

test("uses the exact Intent label in its error", () => {
  const result = validate("", { PR_BODY: "## Summary\n\n## Demo\nN/A docs" });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /after \*\*Intent:\*\* what/);
});

test("rejects an overly verbose Intent", () => {
  const result = validate("", {
    PR_BODY: `## Summary\n\n**Intent:** ${"a".repeat(401)}\n\n## Demo\nN/A docs`,
  });
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /Keep \*\*Intent:\*\* to one or two short sentences/,
  );
});

test("requires the Demo heading", () => {
  const result = validate("", {
    PR_BODY:
      "## Summary\n\n**Intent:** Prevent duplicate notes when the connection recovers.",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Add a ## Demo section/);
});

for (const overrides of [
  { PR_AUTHOR_ASSOCIATION: "OWNER" },
  { PR_AUTHOR_ASSOCIATION: "MEMBER" },
  { PR_AUTHOR_ASSOCIATION: "COLLABORATOR" },
  { PR_AUTHOR_TYPE: "Bot" },
]) {
  test(`preserves the exemption for ${JSON.stringify(overrides)}`, () => {
    const result = validate("", { PR_BODY: "", PR_TITLE: "", ...overrides });
    assert.equal(result.status, 0, result.stderr);
  });
}
