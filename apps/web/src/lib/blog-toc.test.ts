import { compileMDX } from "@content-collections/mdx";
import assert from "node:assert/strict";
import test from "node:test";
import rehypeSlug from "rehype-slug";
import remarkGfm from "remark-gfm";

import {
  type BlogHeading,
  extractBlogHeadingsFromHast,
  rehypeExtractH2Headings,
  shouldRenderBlogToc,
} from "./blog-toc.ts";

test("extracts h2 headings with nested inline text while ignoring other levels and missing ids", () => {
  const tree = {
    type: "root",
    children: [
      {
        type: "element",
        tagName: "h1",
        properties: { id: "page-title" },
        children: [{ type: "text", value: "Page Title" }],
      },
      {
        type: "element",
        tagName: "h2",
        properties: { id: "plain-heading" },
        children: [{ type: "text", value: "  Plain Heading  " }],
      },
      {
        type: "section",
        children: [
          {
            type: "element",
            tagName: "section",
            properties: {},
            children: [
              {
                type: "element",
                tagName: "h2",
                properties: { id: "nested-inline-heading" },
                children: [
                  { type: "text", value: "How " },
                  {
                    type: "element",
                    tagName: "code",
                    children: [{ type: "text", value: "sqlite" }],
                  },
                  { type: "text", value: " and " },
                  {
                    type: "element",
                    tagName: "strong",
                    children: [
                      {
                        type: "element",
                        tagName: "em",
                        children: [{ type: "text", value: "local" }],
                      },
                    ],
                  },
                  { type: "text", value: " " },
                  {
                    type: "element",
                    tagName: "a",
                    properties: { href: "#details" },
                    children: [{ type: "text", value: "sync" }],
                  },
                  { type: "text", value: " work?" },
                ],
              },
            ],
          },
        ],
      },
      {
        type: "element",
        tagName: "h3",
        properties: { id: "sub-heading" },
        children: [{ type: "text", value: "Sub heading" }],
      },
      {
        type: "element",
        tagName: "h2",
        properties: {},
        children: [{ type: "text", value: "Missing id" }],
      },
      {
        type: "element",
        tagName: "h2",
        properties: { id: "   " },
        children: [{ type: "text", value: "Blank id" }],
      },
      {
        type: "element",
        tagName: "h2",
        properties: { id: "empty-text" },
        children: [{ type: "text", value: "   " }],
      },
    ],
  };

  const expected: BlogHeading[] = [
    { id: "plain-heading", text: "Plain Heading" },
    { id: "nested-inline-heading", text: "How sqlite and local sync work?" },
  ];

  assert.deepEqual(extractBlogHeadingsFromHast(tree), expected);
});

test("compiles mixed Markdown and JSX h2 headings alongside legacy anchors", async () => {
  const headings: BlogHeading[] = [];
  const compiled = await compileMDX(
    { cache: async (input, compute) => compute(input) },
    {
      _meta: {
        filePath: "mixed.mdx",
        fileName: "mixed.mdx",
        directory: ".",
        path: "mixed",
        extension: "mdx",
      },
      content: [
        '<span id="legacy-overview" />',
        "",
        "## What changed in the overview?",
        "",
        '<h2 id="jsx-details">How JSX <code>h2</code> sections resolve?</h2>',
        "",
        "<h2>Missing ID should be skipped</h2>",
        "",
        "### Nested h3 should be ignored",
      ].join("\n"),
    },
    {
      remarkPlugins: [remarkGfm],
      rehypePlugins: [rehypeSlug, () => rehypeExtractH2Headings(headings)],
    },
  );

  assert.deepEqual(headings, [
    {
      id: "what-changed-in-the-overview",
      text: "What changed in the overview?",
    },
    { id: "jsx-details", text: "How JSX h2 sections resolve?" },
  ]);
  assert.match(compiled, /id:\s*"legacy-overview"/);
  assert.match(compiled, /id:\s*"what-changed-in-the-overview"/);
  assert.match(compiled, /id:\s*"jsx-details"/);
});

test("renders table of contents only when at least three h2 headings exist", () => {
  assert.equal(
    shouldRenderBlogToc([
      { id: "one", text: "One" },
      { id: "two", text: "Two" },
    ]),
    false,
  );
  assert.equal(
    shouldRenderBlogToc([
      { id: "one", text: "One" },
      { id: "two", text: "Two" },
      { id: "three", text: "Three" },
    ]),
    true,
  );
});
