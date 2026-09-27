import assert from "node:assert/strict";
import test from "node:test";

import { parseInline, parseMarkdownBlocks } from "./markdown-blocks.ts";

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
      "1. First",
      "2. Second",
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
      ordered: false,
      items: [
        {
          spans: [{ text: "Speaker 1 stated they were testing the sound." }],
          checked: undefined,
        },
        {
          spans: [
            { text: "Speaker 3 confirmed it was " },
            { text: "working", bold: true },
            { text: "." },
          ],
          checked: undefined,
        },
      ],
    },
    {
      type: "list",
      ordered: true,
      items: [
        { spans: [{ text: "First" }], checked: undefined },
        { spans: [{ text: "Second" }], checked: undefined },
      ],
    },
    {
      type: "list",
      ordered: false,
      items: [
        { spans: [{ text: "Todo" }], checked: false },
        { spans: [{ text: "Done" }], checked: true },
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
