import assert from "node:assert/strict";
import test from "node:test";

import { modelHasActionButton } from "./action-button-model.ts";

test("recognizes iPhones with an Action Button", () => {
  for (const modelId of [
    "iPhone16,1",
    "iPhone16,2",
    "iPhone17,1",
    "iPhone17,5",
    "iPhone18,3",
    "iPhone19,1",
  ]) {
    assert.equal(modelHasActionButton(modelId), true, modelId);
  }
});

test("hides Action Button setup on devices without one", () => {
  for (const modelId of [
    "iPhone15,4",
    "iPhone15,5",
    "iPhone16,3",
    "iPhone14,7",
    "iPad16,3",
    "arm64",
    "",
    null,
  ]) {
    assert.equal(modelHasActionButton(modelId), false, String(modelId));
  }
});
