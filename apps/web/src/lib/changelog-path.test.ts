import assert from "node:assert/strict";
import test from "node:test";

import { getChangelogReleaseFromPath } from "./changelog-path.ts";

test("excludes all Nightly notes from the website", () => {
  for (const file of [
    "nightly.md",
    "1.4.24-nightly.123.md",
    "1.4.24-beta.1.md",
  ]) {
    assert.equal(
      getChangelogReleaseFromPath(`packages/changelog/content/desktop/${file}`),
      null,
    );
  }
});

test("extracts versions only from changelog release files", () => {
  assert.deepEqual(
    getChangelogReleaseFromPath(
      "../../../../packages/changelog/content/desktop/1.0.32.md",
    ),
    { stream: "desktop", version: "1.0.32" },
  );
  assert.deepEqual(getChangelogReleaseFromPath("content/mobile/1.0.32.md"), {
    stream: "mobile",
    version: "1.0.32",
  });
  assert.equal(getChangelogReleaseFromPath("content/1.0.32.md"), null);
  assert.equal(getChangelogReleaseFromPath("content/unknown/1.0.32.md"), null);
  assert.equal(
    getChangelogReleaseFromPath("packages/changelog/content/AGENTS.md"),
    null,
  );
  assert.equal(
    getChangelogReleaseFromPath("packages/changelog/content/desktop/1.0.md"),
    null,
  );
  assert.equal(
    getChangelogReleaseFromPath(
      "packages/changelog/content/desktop/1.0.32.mdx",
    ),
    null,
  );
});
