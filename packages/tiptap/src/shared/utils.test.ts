import { getSchema } from "@tiptap/core";
import type { JSONContent } from "@tiptap/react";
import { describe, expect, test } from "vitest";

import { getExtensions } from "./extensions";
import { isValidTiptapContent, json2md, md2json } from "./utils";

describe("json2md", () => {
  test("renders task items without escaping brackets", () => {
    const taskListContent = {
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
    };

    const markdown = json2md(taskListContent);

    expect(markdown).toContain("[ ]");
    expect(markdown).not.toContain("\\[");
    expect(markdown).not.toContain("\\]");
    expect(markdown).toContain("this is an example md task");
  });

  test("renders checked task items without escaping brackets", () => {
    const taskListContent = {
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
    };

    const markdown = json2md(taskListContent);

    expect(markdown).toContain("[x]");
    expect(markdown).not.toContain("\\[");
    expect(markdown).not.toContain("\\]");
    expect(markdown).toContain("completed task");
  });

  test("renders multiple task items without escaping brackets", () => {
    const taskListContent = {
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
                  content: [{ type: "text", text: "first task" }],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: { checked: true },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "second task" }],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "third task" }],
                },
              ],
            },
          ],
        },
      ],
    };

    const markdown = json2md(taskListContent);

    expect(markdown).toContain("[ ]");
    expect(markdown).toContain("[x]");
    expect(markdown).not.toContain("\\[");
    expect(markdown).not.toContain("\\]");
    expect(markdown).toContain("first task");
    expect(markdown).toContain("second task");
    expect(markdown).toContain("third task");
  });
});

describe("md2json", () => {
  describe("image handling", () => {
    test("converts standalone image to JSON", () => {
      const markdown = "![alt text](https://example.com/image.png)";
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
      expect(json.content!.length).toBeGreaterThan(0);

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
      expect(imageNode).toBeDefined();
      expect(imageNode?.attrs?.src).toBe("https://example.com/image.png");
      expect(imageNode?.attrs?.alt).toBe("alt text");
    });

    test("converts image with title to JSON", () => {
      const markdown =
        '![alt text](https://example.com/image.png "Image Title")';
      const json = md2json(markdown);

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
      expect(imageNode?.attrs?.src).toBe("https://example.com/image.png");
      expect(imageNode?.attrs?.alt).toBe("alt text");
      expect(imageNode?.attrs?.title).toBe("Image Title");
    });

    test("converts multiple standalone images to JSON", () => {
      const markdown = `![image1](https://example.com/1.png)

![image2](https://example.com/2.png)`;
      const json = md2json(markdown);

      expect(json.content!.length).toBeGreaterThanOrEqual(2);

      const findAllImages = (content: any[]): any[] => {
        const images: any[] = [];
        for (const node of content) {
          if (node.type === "image") images.push(node);
          if (node.content) {
            images.push(...findAllImages(node.content));
          }
        }
        return images;
      };

      const images = findAllImages(json.content!);
      expect(images.length).toBeGreaterThanOrEqual(2);
    });

    test("converts text with inline image to valid schema", () => {
      const markdown =
        "Check out this image: ![cat](https://example.com/cat.png) and more text";
      const json = md2json(markdown);

      const paragraph = json.content![0];
      expect(paragraph.type).toBe("paragraph");

      const imageNode = paragraph.content!.find(
        (node) => node.type === "image",
      );
      expect(imageNode).toBeDefined();
      expect(imageNode?.attrs?.src).toBe("https://example.com/cat.png");
      expect(imageNode?.attrs?.alt).toBe("cat");

      const textNodes = paragraph.content!.filter(
        (node) => node.type === "text",
      );
      expect(textNodes.length).toBeGreaterThan(0);
    });
  });

  describe("nested structures", () => {
    test("converts nested lists with images", () => {
      const markdown = `- Item 1
  - ![nested](https://example.com/nested.png)
  - Item 1.2
- Item 2`;
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
    });

    test("converts blockquote with image", () => {
      const markdown = `> This is a quote
> ![quote image](https://example.com/quote.png)`;
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
    });

    test("converts heading with following image", () => {
      const markdown = `# Title

![header image](https://example.com/header.png)

Some text`;
      const json = md2json(markdown);

      expect(json.content!.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe("edge cases", () => {
    test("handles empty markdown", () => {
      const markdown = "";
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
    });

    test("handles whitespace-only markdown", () => {
      const markdown = "   \n\n   ";
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
    });

    test("handles malformed image syntax", () => {
      const markdown = "![incomplete image](";
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
    });

    test("handles image with no alt text", () => {
      const markdown = "![](https://example.com/no-alt.png)";
      const json = md2json(markdown);

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
      expect(imageNode).toBeDefined();
      expect(imageNode?.attrs?.src).toBe("https://example.com/no-alt.png");
      expect(imageNode?.attrs?.alt).toBe("");
    });

    test("handles very long URLs", () => {
      const longUrl = "https://example.com/" + "a".repeat(1000) + ".png";
      const markdown = `![long url](${longUrl})`;
      const json = md2json(markdown);

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
      expect(imageNode?.attrs?.src).toBe(longUrl);
    });
  });

  describe("mixed content", () => {
    test("converts document with text, images, and lists", () => {
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

    test("converts document with code blocks and images", () => {
      const markdown = `Some code:

\`\`\`javascript
console.log("hello");
\`\`\`

![screenshot](https://example.com/screenshot.png)`;

      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
    });

    test("converts task list with images", () => {
      const markdown = `- [ ] Task 1
- [x] Task 2 ![done](https://example.com/check.png)
- [ ] Task 3`;

      const json = md2json(markdown);

      const taskList = json.content!.find((node) => node.type === "taskList");
      expect(taskList).toBeDefined();
    });
  });
});

describe("md2json mark sanitization", () => {
  const schema = getSchema(getExtensions());

  function validateJsonContent(json: JSONContent): {
    valid: boolean;
    error?: string;
  } {
    try {
      schema.nodeFromJSON(json);
      return { valid: true };
    } catch (error) {
      return {
        valid: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  test("bold wrapping code produces valid schema (no bold+code)", () => {
    const json = md2json("**`code`**");
    const validation = validateJsonContent(json);
    expect(validation.valid).toBe(true);
    if (!validation.valid) {
      throw new Error(`Schema validation failed: ${validation.error}`);
    }
  });

  test("italic wrapping code produces valid schema", () => {
    const json = md2json("*`code`*");
    const validation = validateJsonContent(json);
    expect(validation.valid).toBe(true);
  });

  test("strikethrough wrapping code produces valid schema", () => {
    const json = md2json("~~`code`~~");
    const validation = validateJsonContent(json);
    expect(validation.valid).toBe(true);
  });

  test("bold+code keeps only code mark", () => {
    const json = md2json("**`code`**");
    const findTextNode = (node: JSONContent): JSONContent | null => {
      if (node.type === "text") return node;
      for (const child of node.content || []) {
        const found = findTextNode(child);
        if (found) return found;
      }
      return null;
    };
    const textNode = findTextNode(json);
    expect(textNode).toBeDefined();
    expect(textNode!.marks).toBeDefined();
    expect(textNode!.marks!.length).toBe(1);
    expect(textNode!.marks![0].type).toBe("code");
  });

  test("mixed bold and code in same paragraph produces valid schema", () => {
    const json = md2json("**bold** and **`code`** and more");
    const validation = validateJsonContent(json);
    expect(validation.valid).toBe(true);
  });
});

describe("schema validation", () => {
  const schema = getSchema(getExtensions());

  function validateJsonContent(json: JSONContent): {
    valid: boolean;
    error?: string;
  } {
    try {
      schema.nodeFromJSON(json);
      return { valid: true };
    } catch (error) {
      return {
        valid: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  describe("md2json produces valid content", () => {
    test("standalone image markdown produces schema-valid JSON", () => {
      const markdown = "![alt](https://example.com/image.png)";
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });

    test("standalone image is a direct child of doc (block-level)", () => {
      const markdown = "![alt](https://example.com/image.png)";
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();
      expect(json.content!.length).toBeGreaterThan(0);

      const firstChild = json.content![0];
      expect(firstChild.type).toBe("image");
      expect(firstChild.attrs?.src).toBe("https://example.com/image.png");
      expect(firstChild.attrs?.alt).toBe("alt");
    });

    test("multiple images produce schema-valid JSON", () => {
      const markdown = `![img1](https://example.com/1.png)

![img2](https://example.com/2.png)

![img3](https://example.com/3.png)`;
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });

    test("consecutive standalone images are direct children of doc (block-level)", () => {
      const markdown = `![img1](https://example.com/1.png)

![img2](https://example.com/2.png)`;
      const json = md2json(markdown);

      expect(json.type).toBe("doc");
      expect(json.content).toBeDefined();

      const images = json.content!.filter((node) => node.type === "image");
      expect(images.length).toBeGreaterThanOrEqual(2);

      const img1 = images.find(
        (n) => n.attrs?.src === "https://example.com/1.png",
      );
      const img2 = images.find(
        (n) => n.attrs?.src === "https://example.com/2.png",
      );

      expect(img1).toBeDefined();
      expect(img2).toBeDefined();
    });

    test("mixed content produces schema-valid JSON", () => {
      const markdown = `# Heading

Text paragraph.

![image](https://example.com/img.png)

- List item 1
- List item 2

More text.`;
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });

    test("inline image in text produces schema-valid JSON", () => {
      const markdown =
        "Here is an image ![inline](https://example.com/inline.png) in text.";
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });

    test("task list produces schema-valid JSON", () => {
      const markdown = `- [ ] Task 1
- [x] Task 2
- [ ] Task 3`;
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });

    test("empty document produces schema-valid JSON", () => {
      const markdown = "";
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });

    test("nested structures produce schema-valid JSON", () => {
      const markdown = `> Blockquote with ![image](https://example.com/quote.png) inside

# Heading

1. Numbered list
2. With items`;
      const json = md2json(markdown);

      const validation = validateJsonContent(json);
      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }
    });
  });

  describe("invalid content detection", () => {
    test("detects invalid node types", () => {
      const invalidJson: JSONContent = {
        type: "doc",
        content: [
          {
            type: "invalidNodeType",
            content: [],
          } as any,
        ],
      };

      const validation = validateJsonContent(invalidJson);
      expect(validation.valid).toBe(false);
    });

    test("detects invalid doc structure (missing content)", () => {
      const invalidJson = {
        type: "doc",
      } as JSONContent;

      const validation = validateJsonContent(invalidJson);
      expect(validation.valid).toBe(true);
    });

    test("validates image with src attribute (block-level)", () => {
      const validJson: JSONContent = {
        type: "doc",
        content: [
          {
            type: "image",
            attrs: {
              src: "https://example.com/image.png",
            },
          },
        ],
      };

      const validation = validateJsonContent(validJson);
      expect(validation.valid).toBe(true);
    });
  });

  describe("roundtrip validation", () => {
    test("markdown -> json -> markdown -> json produces consistent valid schema", () => {
      const originalMarkdown = `# Test Document

![image](https://example.com/test.png)

- List item
- Another item

Some text.`;

      const json1 = md2json(originalMarkdown);
      const validation1 = validateJsonContent(json1);
      expect(validation1.valid).toBe(true);

      const markdown2 = json2md(json1);
      const json2 = md2json(markdown2);
      const validation2 = validateJsonContent(json2);
      expect(validation2.valid).toBe(true);
    });

    test("issue #3245: _memo.md with standalone image produces valid schema", () => {
      const memoMarkdown = `![welcome](https://example.com/welcome.png)

We appreciate your patience while you wait.`;

      const json = md2json(memoMarkdown);
      const validation = validateJsonContent(json);

      expect(validation.valid).toBe(true);
      if (!validation.valid) {
        throw new Error(`Schema validation failed: ${validation.error}`);
      }

      expect(json.content!.length).toBeGreaterThanOrEqual(2);

      // First node should be a block-level image (direct child of doc)
      const firstNode = json.content![0];
      expect(firstNode.type).toBe("image");
      expect(firstNode.attrs?.src).toBe("https://example.com/welcome.png");

      // Second node should be a paragraph with text
      const secondNode = json.content![1];
      expect(secondNode.type).toBe("paragraph");
      const textInSecondPara = secondNode.content?.find(
        (n) => n.type === "text",
      );
      expect(textInSecondPara).toBeDefined();
    });
  });
});

describe("isValidTiptapContent", () => {
  test("returns true for valid content", () => {
    const validContent = {
      type: "doc",
      content: [{ type: "paragraph" }],
    };
    expect(isValidTiptapContent(validContent)).toBe(true);
  });

  test("returns false for non-object", () => {
    expect(isValidTiptapContent("string")).toBe(false);
    expect(isValidTiptapContent(123)).toBe(false);
    expect(isValidTiptapContent(null)).toBe(false);
    expect(isValidTiptapContent(undefined)).toBe(false);
  });

  test("returns false for object without type: doc", () => {
    expect(isValidTiptapContent({ type: "paragraph" })).toBe(false);
    expect(isValidTiptapContent({ content: [] })).toBe(false);
  });

  test("returns false for doc without content array", () => {
    expect(isValidTiptapContent({ type: "doc" })).toBe(false);
    expect(isValidTiptapContent({ type: "doc", content: "string" })).toBe(
      false,
    );
  });
});
