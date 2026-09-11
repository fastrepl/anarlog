import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { describe, expect, test } from "vitest";
import type { JSONContent } from "../markdown";
import { md2json } from "../markdown";
import { schema } from "./schema";

const fixturePath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../apps/desktop-gpui/src/editor/pm/fixtures/roundtrip.gen.json",
);

type FixtureDoc = {
  name: string;
  source: "hand" | "markdown";
  input?: string;
  doc: JSONContent;
  nodeSize: number;
  textContent: string;
  textBetween: string;
};

function text(text: string, marks?: JSONContent["marks"]): JSONContent {
  return { type: "text", text, ...(marks ? { marks } : {}) };
}

function paragraph(...content: JSONContent[]): JSONContent {
  return { type: "paragraph", ...(content.length ? { content } : {}) };
}

function mark(type: string, attrs?: Record<string, unknown>) {
  return { type, ...(attrs ? { attrs } : {}) };
}

function hand(name: string, input: JSONContent): FixtureDoc {
  const doc = schema.nodeFromJSON(input);
  doc.check();
  return {
    name,
    source: "hand",
    doc: doc.toJSON() as JSONContent,
    nodeSize: doc.nodeSize,
    textContent: doc.textContent,
    textBetween: doc.textBetween(0, doc.content.size, "\n", "\uFFFC"),
  };
}

function markdown(name: string, input: string): FixtureDoc {
  const raw = md2json(input);
  const doc = schema.nodeFromJSON(raw);
  doc.check();
  return {
    name,
    source: "markdown",
    input,
    doc: doc.toJSON() as JSONContent,
    nodeSize: doc.nodeSize,
    textContent: doc.textContent,
    textBetween: doc.textBetween(0, doc.content.size, "\n", "\uFFFC"),
  };
}

function schemaFixture() {
  const nodes: Record<string, Record<string, unknown>> = {};
  schema.spec.nodes.forEach((name, spec) => {
    const attrs: Record<string, Record<string, unknown>> = {};
    const nodeType = schema.nodes[name];
    for (const [attrName, attrSpec] of Object.entries(spec.attrs ?? {})) {
      const attr: Record<string, unknown> = {};
      if (attrSpec.default !== undefined) attr.default = attrSpec.default;
      attrs[attrName] = attr;
    }
    const node: Record<string, unknown> = {};
    if (spec.content !== undefined) node.content = spec.content;
    if (spec.group !== undefined) node.group = spec.group;
    node.inline = nodeType.isInline;
    node.atom = nodeType.isAtom;
    if (spec.marks !== undefined) node.marks = spec.marks;
    node.attrs = attrs;
    nodes[name] = node;
  });

  const marks: Record<string, Record<string, unknown>> = {};
  schema.spec.marks.forEach((name, spec) => {
    const attrs: Record<string, Record<string, unknown>> = {};
    for (const [attrName, attrSpec] of Object.entries(spec.attrs ?? {})) {
      const attr: Record<string, unknown> = {};
      if (attrSpec.default !== undefined) attr.default = attrSpec.default;
      attrs[attrName] = attr;
    }
    const markSpec: Record<string, unknown> = { attrs };
    if (spec.excludes !== undefined) markSpec.excludes = spec.excludes;
    if (spec.inclusive !== undefined) markSpec.inclusive = spec.inclusive;
    if (spec.group !== undefined) markSpec.group = spec.group;
    marks[name] = markSpec;
  });

  return { nodes, marks, topNode: "doc" };
}

const handDocs: FixtureDoc[] = [
  hand(
    "basic-unicode",
    {
      type: "doc",
      content: [
        paragraph(
          text("Hello, 세계! Café "),
          text("👩‍💻 日本語", [mark("bold"), mark("italic")]),
        ),
      ],
    },
  ),
  hand("empty-paragraph", { type: "doc", content: [paragraph()] }),
  hand("empty-doc", { type: "doc", content: [paragraph()] }),
  hand("headings-all-levels", {
    type: "doc",
    content: [1, 2, 3, 4, 5, 6].map((level) => ({
      type: "heading",
      attrs: { level },
      content: [text(`Heading ${level}`)],
    })),
  }),
  hand("blockquote", {
    type: "doc",
    content: [
      {
        type: "blockquote",
        content: [
          paragraph(text("A quoted paragraph")),
          { type: "paragraph", content: [text("A second paragraph")] },
        ],
      },
    ],
  }),
  hand("code-block-with-language", {
    type: "doc",
    content: [
      {
        type: "codeBlock",
        attrs: { language: "rust" },
        content: [text("fn main() {\n  println!(\"こんにちは\");\n}")],
      },
    ],
  }),
  hand("horizontal-rule-and-hard-break", {
    type: "doc",
    content: [
      paragraph(text("before"), { type: "hardBreak" }, text("after")),
      { type: "horizontalRule" },
    ],
  }),
  hand("nested-lists-three-deep", {
    type: "doc",
    content: [
      {
        type: "bulletList",
        content: [
          {
            type: "listItem",
            content: [
              paragraph(text("level one")),
              {
                type: "orderedList",
                attrs: { start: 4 },
                content: [
                  {
                    type: "listItem",
                    content: [
                      paragraph(text("level two")),
                      {
                        type: "bulletList",
                        content: [
                          {
                            type: "listItem",
                            content: [paragraph(text("level three"))],
                          },
                        ],
                      },
                    ],
                  },
                ],
              },
            ],
          },
        ],
      },
    ],
  }),
  hand("ordered-list-non-default-start", {
    type: "doc",
    content: [
      {
        type: "orderedList",
        attrs: { start: 7 },
        content: [
          { type: "listItem", content: [paragraph(text("seven"))] },
          { type: "listItem", content: [paragraph(text("eight"))] },
        ],
      },
    ],
  }),
  hand("table-with-spans", {
    type: "doc",
    content: [
      {
        type: "table",
        content: [
          {
            type: "tableRow",
            content: [
              {
                type: "tableHeader",
                attrs: { colspan: 2, rowspan: 1, colwidth: [120, 180] },
                content: [paragraph(text("Header"))],
              },
              {
                type: "tableHeader",
                attrs: { colspan: 1, rowspan: 2, colwidth: null },
                content: [paragraph(text("Side"))],
              },
            ],
          },
          {
            type: "tableRow",
            content: [
              {
                type: "tableCell",
                attrs: { colspan: 1, rowspan: 1, colwidth: null },
                content: [paragraph(text("Cell"))],
              },
              {
                type: "tableCell",
                attrs: { colspan: 1, rowspan: 1, colwidth: [90] },
                content: [paragraph(text("Cell 2"))],
              },
            ],
          },
        ],
      },
    ],
  }),
  hand("tasks-checked-and-unchecked", {
    type: "doc",
    content: [
      {
        type: "taskList",
        content: [
          {
            type: "taskItem",
            attrs: { checked: true },
            content: [paragraph(text("Done"))],
          },
          {
            type: "taskItem",
            attrs: { checked: false },
            content: [paragraph(text("Todo"))],
          },
        ],
      },
    ],
  }),
  hand("image-all-attrs", {
    type: "doc",
    content: [
      {
        type: "image",
        attrs: {
          src: "https://example.com/image.png",
          alt: "An image",
          title: "title",
          attachmentId: "attachment-1",
          sharedAttachmentId: "shared-1",
          editorWidth: 72,
        },
      },
    ],
  }),
  hand("file-attachment-all-attrs", {
    type: "doc",
    content: [
      {
        type: "fileAttachment",
        attrs: {
          attachmentId: "attachment-2",
          sharedAttachmentId: "shared-2",
          name: "report.pdf",
          mimeType: "application/pdf",
          src: "asset://report.pdf",
          path: "/tmp/report.pdf",
          size: 4096,
        },
      },
    ],
  }),
  hand("app-link-all-attrs", {
    type: "doc",
    content: [
      paragraph(
        text("See "),
        {
          type: "appLink",
          attrs: {
            provider: "github",
            kind: "issue",
            url: "https://github.com/acme/project/issues/42",
            owner: "acme",
            repo: "project",
            number: 42,
            subId: "sub-1",
            workspace: "acme",
            channelId: "C123",
            messageTs: "123.456",
            threadTs: "123.456",
            guildId: "guild-1",
            messageId: "message-1",
            inviteCode: "invite-1",
            resourceId: "resource-1",
            resourceTitle: "Issue 42",
          },
        },
        text(" "),
      ),
    ],
  }),
  hand("mention-session-and-clip", {
    type: "doc",
    content: [
      paragraph(
        text("Mention: "),
        {
          type: "mention-@",
          attrs: { id: "user-1", type: "human", label: "Ada" },
        },
        text(" "),
      ),
      {
        type: "session",
        attrs: { sessionId: "session-1", status: "done", checked: true },
        content: [paragraph(text("Embedded session"))],
      },
      { type: "clip", attrs: { src: "https://youtube.com/embed/abc" } },
    ],
  }),
  hand("marks-overlap-and-adjacency", {
    type: "doc",
    content: [
      paragraph(
        text("bold italic", [mark("bold"), mark("italic")]),
        text(" adjacent", [mark("bold"), mark("italic")]),
        text(" linked", [
          mark("link", { href: "https://example.com", target: "_blank" }),
          mark("highlight"),
        ]),
        text(" code", [mark("code")]),
      ),
    ],
  }),
  hand("explicit-default-attrs", {
    type: "doc",
    content: [
      {
        type: "orderedList",
        attrs: { start: 1 },
        content: [{ type: "listItem", content: [paragraph(text("one"))] }],
      },
      {
        type: "image",
        attrs: {
          src: null,
          alt: null,
          title: null,
          attachmentId: null,
          sharedAttachmentId: null,
          editorWidth: 80,
        },
      },
    ],
  }),
  hand("default-and-null-attachments", {
    type: "doc",
    content: [
      {
        type: "fileAttachment",
        attrs: { name: "", mimeType: "", attachmentId: null },
      },
      paragraph({
        type: "appLink",
        attrs: { provider: "github", url: null, kind: null },
      }),
    ],
  }),
  hand("all-block-types", {
    type: "doc",
    content: [
      paragraph(text("paragraph")),
      { type: "horizontalRule" },
      paragraph({
        type: "appLink",
        attrs: { provider: "github", kind: "issue", url: null },
      }),
      { type: "clip", attrs: { src: null } },
      {
        type: "fileAttachment",
        attrs: { name: "file.txt", mimeType: "text/plain" },
      },
    ],
  }),
];

const markdownDocs = [
  markdown(
    "markdown-headings-and-marks",
    "# Heading\n\n**bold** *italic* ~~strike~~ ==highlight== [link](https://example.com)",
  ),
  markdown(
    "markdown-blocks-and-lists",
    "> quote\n>\n> second\n\n- one\n- two\n\n1. three\n2. four",
  ),
  markdown(
    "markdown-tasks-and-breaks",
    "- [ ] Todo\n- [x] Done\n\nfirst  \nsecond",
  ),
  markdown(
    "markdown-table",
    "| Name | Value |\n| --- | --- |\n| 日本語 | 42 |\n| emoji | 👩‍💻 |",
  ),
  markdown(
    "markdown-image-and-attachment",
    "![alt](https://example.com/a.png \"title\")\n\n[report.pdf](attachment://report.pdf)",
  ),
  markdown(
    "markdown-code-and-horizontal-rule",
    "```rust\nfn main() {}\n```\n\n---\n\n`inline code`",
  ),
];

const fixture = {
  schema: schemaFixture(),
  docs: [...handDocs, ...markdownDocs],
};

describe("editor round-trip fixture", () => {
  test("matches the checked-in fixture", async () => {
    const generated = `${JSON.stringify(fixture, null, 2)}\n`;
    if (process.env.UPDATE_ROUNDTRIP_FIXTURES === "1") {
      await mkdir(dirname(fixturePath), { recursive: true });
      await writeFile(fixturePath, generated);
      return;
    }

    const checkedIn = await readFile(fixturePath, "utf8");
    expect(
      checkedIn,
      "round-trip fixture drifted; regenerate with UPDATE_ROUNDTRIP_FIXTURES=1",
    ).toBe(generated);
  });
});
