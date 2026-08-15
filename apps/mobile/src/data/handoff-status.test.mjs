import assert from "node:assert/strict";
import test from "node:test";

import { handoffStatusCopy } from "./handoff-status.ts";

test("handoff copy never claims desktop delivery without confirmation", () => {
  assert.match(handoffStatusCopy("local"), /On this phone/);
  assert.match(handoffStatusCopy("shared_unconfirmed"), /not confirmed/);
  assert.doesNotMatch(handoffStatusCopy("shared_unconfirmed"), /synced/i);
});

test("failed handoff keeps the local recording recovery explicit", () => {
  assert.match(handoffStatusCopy("failed"), /still safe on this phone/);
});
