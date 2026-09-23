import assert from "node:assert/strict";
import test from "node:test";

import {
  EXPERIMENTS,
  experimentProperties,
  resolveExperimentVariant,
} from "./experiments.ts";

test("falls back to the first variant when the flag is missing or unknown", () => {
  assert.equal(
    resolveExperimentVariant("downloadLayout", undefined),
    "control",
  );
  assert.equal(resolveExperimentVariant("downloadLayout", false), "control");
  assert.equal(resolveExperimentVariant("downloadLayout", "bogus"), "control");
});

test("returns declared variants verbatim", () => {
  assert.equal(
    resolveExperimentVariant("downloadLayout", "three-column"),
    "three-column",
  );
  assert.equal(
    resolveExperimentVariant("downloadLayout", "control"),
    "control",
  );
});

test("every experiment lists control first with a unique flag key", () => {
  const flags = new Set<string>();
  for (const { flag, variants } of Object.values(EXPERIMENTS)) {
    assert.equal(variants[0], "control");
    assert.ok(/^web-[a-z0-9-]+$/.test(flag), flag);
    assert.ok(!flags.has(flag), `duplicate flag ${flag}`);
    flags.add(flag);
  }
});

test("experiment properties are analytics-safe tokens", () => {
  assert.deepEqual(experimentProperties("downloadLayout", "three-column"), {
    experiment: "web-download-layout",
    variant: "three-column",
  });
});
