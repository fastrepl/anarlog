import assert from "node:assert/strict";
import test from "node:test";

import { parseInline, parseMarkdownBlocks } from "./markdown-blocks.ts";

const item = (spans, extra = {}) => ({
  spans,
  depth: 0,
  number: undefined,
  checked: undefined,
  ...extra,
});

test("parses headings, paragraphs, and lists", () => {
  const blocks = parseMarkdownBlocks(
    [
      "## Overview",
      "This recording consists solely of an audio check.",
      "",
      "## Discussion Details",
      "- Speaker 1 stated they were testing the sound.",
      "- Speaker 3 confirmed it was **working**.",
      "",
      "3. Third",
      "4. Fourth",
      "",
      "- [ ] Todo",
      "- [x] Done",
      "",
      "**Note:** The transcript is short.",
    ].join("\n"),
  );
  assert.deepEqual(blocks, [
    { type: "heading", level: 2, spans: [{ text: "Overview" }] },
    {
      type: "paragraph",
      spans: [{ text: "This recording consists solely of an audio check." }],
    },
    { type: "heading", level: 2, spans: [{ text: "Discussion Details" }] },
    {
      type: "list",
      items: [
        item([{ text: "Speaker 1 stated they were testing the sound." }]),
        item([
          { text: "Speaker 3 confirmed it was " },
          { text: "working", bold: true },
          { text: "." },
        ]),
      ],
    },
    {
      type: "list",
      items: [
        item([{ text: "Third" }], { number: 3 }),
        item([{ text: "Fourth" }], { number: 4 }),
      ],
    },
    {
      type: "list",
      items: [
        item([{ text: "Todo" }], { checked: false }),
        item([{ text: "Done" }], { checked: true }),
      ],
    },
    {
      type: "paragraph",
      spans: [
        { text: "Note:", bold: true },
        { text: " The transcript is short." },
      ],
    },
  ]);
});

test("keeps nested list depth", () => {
  assert.deepEqual(
    parseMarkdownBlocks("- Parent\n  - Child\n    1. Grandchild\n- Sibling"),
    [
      {
        type: "list",
        items: [
          item([{ text: "Parent" }]),
          item([{ text: "Child" }], { depth: 1 }),
          item([{ text: "Grandchild" }], { depth: 2, number: 1 }),
          item([{ text: "Sibling" }]),
        ],
      },
    ],
  );
});

test("counts repeated ordered markers and four-space indents", () => {
  assert.deepEqual(
    parseMarkdownBlocks("1. One\n1. Two\n    - Child\n    - Child 2\n1. Three"),
    [
      {
        type: "list",
        items: [
          item([{ text: "One" }], { number: 1 }),
          item([{ text: "Two" }], { number: 2 }),
          item([{ text: "Child" }], { depth: 1 }),
          item([{ text: "Child 2" }], { depth: 1 }),
          item([{ text: "Three" }], { number: 3 }),
        ],
      },
    ],
  );
});

test("joins wrapped paragraph lines and skips rules", () => {
  assert.deepEqual(parseMarkdownBlocks("one\ntwo\n\n---\n\nthree"), [
    { type: "paragraph", spans: [{ text: "one two" }] },
    { type: "paragraph", spans: [{ text: "three" }] },
  ]);
});

test("parses inline emphasis and code", () => {
  assert.deepEqual(parseInline("a *b* _c_ `d` __e__"), [
    { text: "a " },
    { text: "b", italic: true },
    { text: " " },
    { text: "c", italic: true },
    { text: " " },
    { text: "d", code: true },
    { text: " " },
    { text: "e", bold: true },
  ]);
  assert.deepEqual(parseInline("plain text"), [{ text: "plain text" }]);
});
