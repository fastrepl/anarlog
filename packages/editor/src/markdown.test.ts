import { describe, expect, test } from "vitest";

import {
  EMPTY_DOC,
  isValidContent,
  json2md,
  type JSONContent,
  md2json,
  parseJsonContent,
} from "./markdown";

describe("json2md", () => {
  test("renders underline as html tags", () => {
    const markdown = json2md({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "underlined",
              marks: [{ type: "underline" }],
            },
          ],
        },
      ],
    });

    expect(markdown).toBe("<u>underlined</u>");
  });

  test("renders task items without escaping brackets", () => {
    const markdown = json2md({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [
                    { type: "text", text: "this is an example md task" },
                  ],
                },
              ],
            },
          ],
        },
      ],
    });

    expect(markdown).toContain("[ ]");
    expect(markdown).not.toContain("\\[");
    expect(markdown).not.toContain("\\]");
    expect(markdown).toContain("this is an example md task");
  });

  test("renders checked task items", () => {
    const markdown = json2md({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: true },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "completed task" }],
                },
              ],
            },
          ],
        },
      ],
    });

    expect(markdown).toContain("[x]");
    expect(markdown).toContain("completed task");
  });

  test("renders image width metadata into markdown titles", () => {
    const markdown = json2md({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            src: "https://example.com/image.png",
            alt: "alt text",
            title: "Example",
            editorWidth: 42,
          },
        },
      ],
    });

    expect(markdown).toBe(
      '![alt text](https://example.com/image.png "char-editor-width=42|Example")',
    );
  });
});

describe("md2json", () => {
  test("converts html underline tags to underline marks", () => {
    const json = md2json("<u>underlined</u>");
    const paragraph = json.content?.[0];
    const textNode = paragraph?.content?.[0];

    expect(paragraph?.type).toBe("paragraph");
    expect(textNode?.type).toBe("text");
    expect(textNode?.text).toBe("underlined");
    expect(textNode?.marks).toEqual([{ type: "underline" }]);
  });

  test("converts standalone image to block-level JSON", () => {
    const json = md2json("![alt text](https://example.com/image.png)");

    expect(json.type).toBe("doc");
    expect(json.content![0].type).toBe("image");
    expect(json.content![0].attrs?.src).toBe("https://example.com/image.png");
    expect(json.content![0].attrs?.alt).toBe("alt text");
    expect(json.content![0].attrs?.editorWidth).toBe(80);
  });

  test("converts image with title to JSON", () => {
    const json = md2json(
      '![alt text](https://example.com/image.png "Image Title")',
    );

    const findImage = (content: any[]): any => {
      for (const node of content) {
        if (node.type === "image") return node;
        if (node.content) {
          const found = findImage(node.content);
          if (found) return found;
        }
      }
      return null;
    };

    const imageNode = findImage(json.content!);
    expect(imageNode?.attrs?.title).toBe("Image Title");
    expect(imageNode?.attrs?.editorWidth).toBe(80);
  });

  test("converts image width metadata to JSON attributes", () => {
    const json = md2json(
      '![alt text](https://example.com/image.png "char-editor-width=42|Image Title")',
    );

    const findImage = (content: any[]): any => {
      for (const node of content) {
        if (node.type === "image") return node;
        if (node.content) {
          const found = findImage(node.content);
          if (found) return found;
        }
      }
      return null;
    };

    const imageNode = findImage(json.content!);
    expect(imageNode?.attrs?.title).toBe("Image Title");
    expect(imageNode?.attrs?.editorWidth).toBe(42);
  });

  test("handles empty markdown", () => {
    const json = md2json("");
    expect(json.type).toBe("doc");
    expect(json.content).toBeDefined();
  });

  test("converts task list", () => {
    const json = md2json("- [ ] Task 1\n- [x] Task 2\n- [ ] Task 3");

    const taskList = json.content!.find((node) => node.type === "taskList");
    expect(taskList).toBeDefined();
  });

  test("converts mixed content document", () => {
    const markdown = `# Introduction

Here is some text.

![diagram](https://example.com/diagram.png)

- List item 1
- List item 2

More text here.`;

    const json = md2json(markdown);
    expect(json.type).toBe("doc");
    expect(json.content!.length).toBeGreaterThan(3);
  });

  test("standalone image with following text produces correct structure", () => {
    const json = md2json(`![welcome](https://example.com/welcome.png)

We appreciate your patience while you wait.`);

    expect(json.content!.length).toBeGreaterThanOrEqual(2);
    expect(json.content![0].type).toBe("image");
    expect(json.content![0].attrs?.src).toBe("https://example.com/welcome.png");
    expect(json.content![1].type).toBe("paragraph");
  });
});

describe("roundtrip", () => {
  test("markdown -> json -> markdown -> json produces consistent results", () => {
    const originalMarkdown = `# Test Document

![image](https://example.com/test.png)

- List item
- Another item

Some text.`;

    const json1 = md2json(originalMarkdown);
    const markdown2 = json2md(json1);
    const json2 = md2json(markdown2);

    expect(json1.type).toBe("doc");
    expect(json2.type).toBe("doc");
    expect(json1.content!.length).toBe(json2.content!.length);
  });

  test("preserves empty paragraphs across roundtrip", () => {
    const json1: JSONContent = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "first" }],
        },
        { type: "paragraph" },
        { type: "paragraph" },
        {
          type: "paragraph",
          content: [{ type: "text", text: "second" }],
        },
      ],
    };

    const markdown = json2md(json1);
    const json2 = md2json(markdown);

    expect(json2.content!.length).toBe(4);
    expect(json2.content![0].content?.[0]?.text).toBe("first");
    expect(json2.content![1].content).toBeUndefined();
    expect(json2.content![2].content).toBeUndefined();
    expect(json2.content![3].content?.[0]?.text).toBe("second");
  });

  test("serializes empty paragraphs as extra blank lines", () => {
    const markdown = json2md({
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "a" }] },
        { type: "paragraph" },
        { type: "paragraph", content: [{ type: "text", text: "b" }] },
      ],
    });

    // 1 empty paragraph between = 2 blank lines = 3 consecutive newlines
    expect(markdown).toContain("a\n\n\nb");
    expect(markdown).not.toContain("&nbsp;");
    expect(markdown).not.toContain("\u00A0");
  });

  test("preserves multiple consecutive empty paragraphs", () => {
    const json1: JSONContent = {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "a" }] },
        { type: "paragraph" },
        { type: "paragraph" },
        { type: "paragraph", content: [{ type: "text", text: "b" }] },
      ],
    };
    const markdown = json2md(json1);
    const json2 = md2json(markdown);

    expect(json2.content!.length).toBe(4);
    expect(json2.content![1].content).toBeUndefined();
    expect(json2.content![2].content).toBeUndefined();
  });

  test("preserves leading empty paragraphs", () => {
    const json1: JSONContent = {
      type: "doc",
      content: [
        { type: "paragraph" },
        { type: "paragraph" },
        { type: "paragraph", content: [{ type: "text", text: "hello" }] },
      ],
    };
    const markdown = json2md(json1);
    const json2 = md2json(markdown);

    expect(json2.content!.length).toBe(3);
    expect(json2.content![0].content).toBeUndefined();
    expect(json2.content![1].content).toBeUndefined();
    expect(json2.content![2].content?.[0]?.text).toBe("hello");
  });

  test("preserves trailing empty paragraphs", () => {
    const json1: JSONContent = {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "hello" }] },
        { type: "paragraph" },
        { type: "paragraph" },
      ],
    };
    const markdown = json2md(json1);
    const json2 = md2json(markdown);

    expect(json2.content!.length).toBe(3);
    expect(json2.content![0].content?.[0]?.text).toBe("hello");
    expect(json2.content![1].content).toBeUndefined();
    expect(json2.content![2].content).toBeUndefined();
  });

  test("parses leading blank lines from raw markdown", () => {
    const json = md2json("\n\nhello");
    expect(json.content!.length).toBe(3);
    expect(json.content![0].content).toBeUndefined();
    expect(json.content![1].content).toBeUndefined();
    expect(json.content![2].content?.[0]?.text).toBe("hello");
  });
});

describe("isValidContent", () => {
  test("returns true for valid content", () => {
    expect(
      isValidContent({ type: "doc", content: [{ type: "paragraph" }] }),
    ).toBe(true);
  });

  test("returns false for non-object", () => {
    expect(isValidContent("string")).toBe(false);
    expect(isValidContent(null)).toBe(false);
    expect(isValidContent(undefined)).toBe(false);
  });

  test("returns false for doc without content array", () => {
    expect(isValidContent({ type: "doc" })).toBe(false);
  });
});

describe("parseJsonContent", () => {
  test("parses valid JSON string", () => {
    const raw = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
    const result = parseJsonContent(raw);
    expect(result.type).toBe("doc");
  });

  test("returns EMPTY_DOC for empty input", () => {
    expect(parseJsonContent("")).toEqual(EMPTY_DOC);
    expect(parseJsonContent(null)).toEqual(EMPTY_DOC);
    expect(parseJsonContent(undefined)).toEqual(EMPTY_DOC);
  });
});

describe("fileAttachment round-trip", () => {
  test("serializes fileAttachment node to markdown link", () => {
    const md = json2md({
      type: "doc",
      content: [
        {
          type: "fileAttachment",
          attrs: {
            name: "report.pdf",
            src: "asset://localhost/%2Fpath%2Freport.pdf",
          },
        },
      ],
    });
    expect(md).toBe("[report.pdf](asset://localhost/%2Fpath%2Freport.pdf)");
  });

  test("parses markdown link with asset:// to fileAttachment", () => {
    const json = md2json(
      "[report.pdf](asset://localhost/%2Fpath%2Freport.pdf)",
    );
    const attachments = json.content!.filter(
      (n) => n.type === "fileAttachment",
    );
    expect(attachments).toHaveLength(1);
    expect(attachments[0].attrs?.name).toBe("report.pdf");
    expect(attachments[0].attrs?.src).toBe(
      "asset://localhost/%2Fpath%2Freport.pdf",
    );
  });

  test("round-trips two file attachments without leaking URL tail", () => {
    const doc: JSONContent = {
      type: "doc",
      content: [
        {
          type: "fileAttachment",
          attrs: {
            name: "CE2 The devil wears Prada script.pdf",
            src: "asset://localhost/%2FUsers%2Fsungbin%2FLibrary%2FApplication%20Support%2Fcom.hyprnote.dev%2Fsessions%2Ff515cc6f%2Fattachments%2FCE2%20The%20devil%20wears%20Prada%20script%202.pdf",
          },
        },
        {
          type: "fileAttachment",
          attrs: {
            name: "2021-13630 조성빈 물리학 1 HW2.pdf",
            src: "asset://localhost/%2FUsers%2Fsungbin%2FLibrary%2FApplication%20Support%2Fcom.hyprnote.dev%2Fsessions%2Ff515cc6f%2Fattachments%2F2021-13630%20%E1%84%8C%E1%85%A9%E1%84%89%E1%85%A5%E1%86%BC%E1%84%87%E1%85%B5%E1%86%AB%20%E1%84%86%E1%85%AE%E1%86%AF%E1%84%85%E1%85%B5%E1%84%92%E1%85%A1%E1%86%A8%201%20HW2.pdf",
          },
        },
      ],
    };

    const md = json2md(doc);
    const parsed = md2json(md);

    const attachments = parsed.content!.filter(
      (n) => n.type === "fileAttachment",
    );
    expect(attachments).toHaveLength(2);
    expect(attachments[0].attrs?.name).toBe(
      "CE2 The devil wears Prada script.pdf",
    );
    expect(attachments[1].attrs?.name).toBe(
      "2021-13630 조성빈 물리학 1 HW2.pdf",
    );
    expect(attachments[0].attrs?.src).toBe(doc.content![0].attrs!.src);
    expect(attachments[1].attrs?.src).toBe(doc.content![1].attrs!.src);

    const leakedText = parsed
      .content!.filter((n) => n.type === "paragraph")
      .flatMap((p) => p.content ?? [])
      .filter((n) => n.type === "text")
      .map((n) => n.text)
      .join("");
    expect(leakedText).toBe("");
  });

  test("handles parentheses in filename via percent-encoding", () => {
    const doc: JSONContent = {
      type: "doc",
      content: [
        {
          type: "fileAttachment",
          attrs: {
            name: "CE2 (Group 5) PPT.pdf",
            src: "asset://localhost/%2Fpath%2FCE2%20(Group%205)%20PPT.pdf",
          },
        },
      ],
    };

    const md = json2md(doc);
    // Parens in URL must be encoded so the markdown link syntax is unambiguous.
    expect(md).toContain("%28Group%205%29");

    const parsed = md2json(md);
    const attachments = parsed.content!.filter(
      (n) => n.type === "fileAttachment",
    );
    expect(attachments).toHaveLength(1);
    expect(attachments[0].attrs?.name).toBe("CE2 (Group 5) PPT.pdf");
  });
});
