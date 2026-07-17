import assert from "node:assert/strict";
import test from "node:test";

import { getChangelogVersionFromPath } from "./changelog-path.ts";

test("extracts versions only from changelog release files", () => {
  assert.equal(
    getChangelogVersionFromPath(
      "../../../../packages/changelog/content/1.0.32.md",
    ),
    "1.0.32",
  );
  assert.equal(
    getChangelogVersionFromPath("packages/changelog/content/AGENTS.md"),
    null,
  );
  assert.equal(
    getChangelogVersionFromPath("packages/changelog/content/1.0.md"),
    null,
  );
  assert.equal(
    getChangelogVersionFromPath("packages/changelog/content/1.0.32.mdx"),
    null,
  );
});
