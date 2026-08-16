import assert from "node:assert/strict";
import test from "node:test";

import { applyEditorFormat } from "./editor-format.ts";

test("wraps and unwraps selected inline text", () => {
  const bold = applyEditorFormat("hello world", { start: 0, end: 5 }, "bold");
  assert.deepEqual(bold, {
    text: "**hello** world",
    selection: { start: 2, end: 7 },
  });
  assert.deepEqual(applyEditorFormat(bold.text, bold.selection, "bold"), {
    text: "hello world",
    selection: { start: 0, end: 5 },
  });
});

test("inserts paired inline markers at the cursor", () => {
  assert.deepEqual(
    applyEditorFormat("hello ", { start: 6, end: 6 }, "italic"),
    {
      text: "hello __",
      selection: { start: 7, end: 7 },
    },
  );
});

test("toggles a heading on the current line", () => {
  const heading = applyEditorFormat(
    "first\nsecond",
    { start: 8, end: 8 },
    "heading",
  );
  assert.deepEqual(heading, {
    text: "first\n# second",
    selection: { start: 10, end: 10 },
  });
  assert.deepEqual(
    applyEditorFormat(heading.text, heading.selection, "heading"),
    {
      text: "first\nsecond",
      selection: { start: 8, end: 8 },
    },
  );
});

test("switches line formats instead of stacking prefixes", () => {
  assert.deepEqual(
    applyEditorFormat("- item", { start: 3, end: 3 }, "checklist"),
    {
      text: "- [ ] item",
      selection: { start: 7, end: 7 },
    },
  );
});

test("formats every selected line", () => {
  assert.deepEqual(
    applyEditorFormat("one\ntwo\nthree", { start: 0, end: 7 }, "bullet"),
    {
      text: "- one\n- two\nthree",
      selection: { start: 0, end: 11 },
    },
  );
});
