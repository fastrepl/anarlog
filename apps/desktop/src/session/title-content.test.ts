import { describe, expect, it } from "vitest";

import {
  ensureFirstLineTitle,
  ensureMarkdownFirstLineTitle,
  extractFirstLineTitle,
} from "./title-content";

describe("extractFirstLineTitle", () => {
  it("returns the first block text", () => {
    expect(
      extractFirstLineTitle({
        type: "doc",
        content: [
          {
            type: "heading",
            attrs: { level: 1 },
            content: [{ type: "text", text: "Planning" }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Follow up" }],
          },
        ],
      }),
    ).toBe("Planning");
  });

  it("returns an empty title when the body has content but the title is blank", () => {
    expect(
      extractFirstLineTitle({
        type: "doc",
        content: [
          { type: "heading", attrs: { level: 1 } },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Follow up" }],
          },
        ],
      }),
    ).toBe("");
  });

  it("does not update titles for an empty document", () => {
    expect(
      extractFirstLineTitle({
        type: "doc",
        content: [{ type: "heading", attrs: { level: 1 } }],
      }),
    ).toBeNull();
  });
});

describe("ensureFirstLineTitle", () => {
  it("prepends the session title before generated summary headings", () => {
    expect(
      ensureFirstLineTitle(
        {
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 1 },
              content: [{ type: "text", text: "Summary Section" }],
            },
          ],
        },
        "Meeting Title",
      ),
    ).toEqual({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Meeting Title" }],
        },
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Summary Section" }],
        },
      ],
    });
  });

  it("converts an existing first paragraph title without duplicating it", () => {
    expect(
      ensureFirstLineTitle(
        {
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "Meeting Title" }],
            },
            {
              type: "paragraph",
              content: [{ type: "text", text: "Follow up" }],
            },
          ],
        },
        "Meeting Title",
      ),
    ).toEqual({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Meeting Title" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    });
  });

  it("preserves a non-matching first heading by prepending the session title", () => {
    expect(
      ensureFirstLineTitle(
        {
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 1 },
              content: [{ type: "text", text: "Old Title" }],
            },
            {
              type: "paragraph",
              content: [{ type: "text", text: "Follow up" }],
            },
          ],
        },
        "Meeting Title",
      ),
    ).toEqual({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Meeting Title" }],
        },
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Old Title" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    });
  });

  it("does not duplicate an existing title line", () => {
    const content = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Meeting Title" }],
        },
      ],
    };

    expect(ensureFirstLineTitle(content, "Meeting Title")).toBe(content);
  });
});

describe("ensureMarkdownFirstLineTitle", () => {
  it("prepends the session title before markdown summary headings", () => {
    expect(
      ensureMarkdownFirstLineTitle(
        "# Summary Section\n\n- Follow up",
        "Meeting Title",
      ),
    ).toBe("# Meeting Title\n\n# Summary Section\n\n- Follow up");
  });

  it("does not duplicate an exact markdown heading without a trailing newline", () => {
    expect(
      ensureMarkdownFirstLineTitle("# Meeting Title", "Meeting Title"),
    ).toBe("# Meeting Title");
  });
});
