import assert from "node:assert/strict";
import test from "node:test";

import { scrollVisibility } from "./scroll-visibility.ts";

const visible = { anchor: 0, hidden: false };

test("hides after scrolling down and reveals after reversing direction", () => {
  let state = scrollVisibility(visible, 8, 500);
  assert.equal(state.hidden, false);
  state = scrollVisibility(state, 12, 500);
  assert.equal(state.hidden, true);
  state = scrollVisibility(state, 250, 500);
  state = scrollVisibility(state, 240, 500);
  assert.equal(state.hidden, true);
  state = scrollVisibility(state, 238, 500);
  assert.equal(state.hidden, false);
  state = scrollVisibility(state, 250, 500);
  assert.equal(state.hidden, true);
});

test("small scroll jitter does not flicker the button", () => {
  let state = scrollVisibility(visible, 100, 500);
  for (const offset of [98, 102, 99, 103, 101, 97]) {
    state = scrollVisibility(state, offset, 500);
    assert.equal(state.hidden, true);
  }
  state = scrollVisibility(state, 90, 500);
  assert.equal(state.hidden, false);
  for (const offset of [92, 87, 90, 89, 96]) {
    state = scrollVisibility(state, offset, 500);
    assert.equal(state.hidden, false);
  }
});

test("top bounce keeps the button visible", () => {
  let state = scrollVisibility(visible, 100, 500);
  for (const offset of [0, -40, -10, 0]) {
    state = scrollVisibility(state, offset, 500);
    assert.equal(state.hidden, false);
  }
});

test("bottom bounce does not count as scrolling back up", () => {
  let state = visible;
  for (const offset of [500, 540, 520, 500]) {
    state = scrollVisibility(state, offset, 500);
    assert.equal(state.hidden, true);
  }
  state = scrollVisibility(state, 488, 500);
  assert.equal(state.hidden, false);
});

test("short or emptied lists always keep the button available", () => {
  let state = scrollVisibility(visible, 100, 500);
  for (const maxOffset of [0, -200]) {
    for (const offset of [100, -50, 0]) {
      state = scrollVisibility(state, offset, maxOffset);
      assert.equal(state.hidden, false);
    }
  }
});
