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

test("bounces and short lists never hide the button spuriously", () => {
  let state = scrollVisibility(visible, 100, 500);
  for (const offset of [0, -40, -10, 0]) {
    state = scrollVisibility(state, offset, 500);
    assert.equal(state.hidden, false);
  }

  state = visible;
  for (const offset of [500, 540, 520, 500]) {
    state = scrollVisibility(state, offset, 500);
    assert.equal(state.hidden, true);
  }
  state = scrollVisibility(state, 488, 500);
  assert.equal(state.hidden, false);

  state = scrollVisibility(visible, 100, 500);
  for (const maxOffset of [0, -200]) {
    for (const offset of [100, -50, 0]) {
      state = scrollVisibility(state, offset, maxOffset);
      assert.equal(state.hidden, false);
    }
  }
});
