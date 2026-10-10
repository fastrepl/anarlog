import { describe, expect, it } from "vitest";

import { schema } from "@anlg/editor/note";

import {
  documentTitlePlaceholder,
  ensureFirstLineTitle,
  extractBodyHeadingsKey,
  extractFirstLineTitle,
  extractSectionHeadings,
  removeDocumentTitle,
} from "./title-content";

describe("documentTitlePlaceholder", () => {
  it("shows Untitled only for the document title block", () => {
    expect(
      documentTitlePlaceholder({
        node: schema.node("heading", { level: 1 }),
        pos: 0,
        hasAnchor: true,
      }),
    ).toBe("Untitled");
    expect(
      documentTitlePlaceholder({
        node: schema.node("paragraph"),
        pos: 2,
        hasAnchor: true,
      }),
    ).toBe("");
  });
});

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

describe("removeDocumentTitle", () => {
  it("removes a legacy title block that matches the session title", () => {
    expect(
      removeDocumentTitle(
        {
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
        },
        "Planning",
      ),
    ).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    });
  });

  it("keeps a first heading that is part of the memo body", () => {
    const content = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Agenda" }],
        },
      ],
    };

    expect(removeDocumentTitle(content, "Planning")).toBe(content);
  });

  it("leaves an empty paragraph after removing an empty title", () => {
    expect(
      removeDocumentTitle(
        {
          type: "doc",
          content: [{ type: "heading", attrs: { level: 1 } }],
        },
        "",
      ),
    ).toEqual({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
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

describe("extractSectionHeadings", () => {
  it("returns trimmed level-two headings and skips the rest", () => {
    expect(
      extractSectionHeadings({
        type: "doc",
        content: [
          {
            type: "heading",
            attrs: { level: 1 },
            content: [{ type: "text", text: "Title" }],
          },
          {
            type: "heading",
            attrs: { level: 2 },
            content: [
              { type: "text", text: "  Updates " },
              { type: "text", text: " & notes" },
            ],
          },
          { type: "heading", attrs: { level: 2 } },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Body" }],
          },
        ],
      }),
    ).toEqual(["Updates  & notes"]);
  });
});

describe("extractBodyHeadingsKey", () => {
  it("joins headings from JSON and markdown bodies and rejects garbage", () => {
    expect(
      extractBodyHeadingsKey(
        JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Next Steps" }],
            },
          ],
        }),
      ),
    ).toBe("Updates\nNext Steps");
    expect(extractBodyHeadingsKey("## Updates\n", "markdown")).toBe("Updates");
    expect(extractBodyHeadingsKey("not json")).toBeNull();
  });
});
