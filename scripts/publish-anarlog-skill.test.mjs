import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  CANONICAL_SKILL_PATH,
  PUBLISHED_SKILL_PATH,
  publishSkill,
  REFERENCE_LINK_REWRITES,
} from "./publish-anarlog-skill.mjs";

test("published skill equals the canonical skill after only link rewrites", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  const published = await readFile(PUBLISHED_SKILL_PATH, "utf8");

  assert.equal(published, publishSkill(canonical));
});

test("the transformation changes nothing but the defined reference links", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  let restored = publishSkill(canonical);
  for (const [reference, url] of Object.entries(REFERENCE_LINK_REWRITES)) {
    restored = restored.split(`](${url})`).join(`](${reference})`);
  }

  assert.equal(restored, canonical);
});

test("non-link drift in the published mirror is detected", async () => {
  const canonical = await readFile(CANONICAL_SKILL_PATH, "utf8");
  const published = await readFile(PUBLISHED_SKILL_PATH, "utf8");

  assert.notEqual(
    publishSkill(canonical.replace("read-only", "writable")),
    published,
  );
});

test("reference links without a public mapping fail publishing", () => {
  assert.throws(
    () => publishSkill("see [new page](references/new-page.md)"),
    /no public URL mapping/,
  );
});
