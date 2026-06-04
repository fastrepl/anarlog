import type { Node as ProseMirrorNode } from "prosemirror-model";
import { EditorState } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import { autolinkPlugin } from "./autolink";
import { linkBoundaryGuardPlugin } from "./link-boundary-guard";

describe("autolinkPlugin", () => {
  it("links bare domains with an https href", () => {
    const state = insertText("x.com");

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "x.com",
              marks: [
                {
                  type: "link",
                  attrs: { href: "https://x.com", target: null },
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("links paths without swallowing trailing sentence punctuation", () => {
    const state = insertText(
      "See linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview.",
    );

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            { type: "text", text: "See " },
            {
              type: "text",
              text: "linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview",
              marks: [
                {
                  type: "link",
                  attrs: {
                    href: "https://linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview",
                    target: null,
                  },
                },
              ],
            },
            { type: "text", text: "." },
          ],
        },
      ],
    });
  });

  it("does not link email domains", () => {
    const state = insertText("email support@x.com");

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "email support@x.com" }],
        },
      ],
    });
  });

  it("extends a bare-domain link when adjacent path text is typed", () => {
    let state = insertText("x.com");
    const insertPos = state.doc.content.size - 1;
    state = state.applyTransaction(
      state.tr.insertText("/getcharnotes", insertPos),
    ).state;

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "x.com/getcharnotes",
              marks: [
                {
                  type: "link",
                  attrs: {
                    href: "https://x.com/getcharnotes",
                    target: null,
                  },
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("updates the link href when linked URL text changes", () => {
    let state = insertText("x.com");
    state = state.applyTransaction(state.tr.insertText("y.com", 1, 6)).state;

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "y.com",
              marks: [
                {
                  type: "link",
                  attrs: {
                    href: "https://y.com",
                    target: null,
                  },
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("keeps a custom href when unrelated text changes", () => {
    const link = schema.marks.link.create({
      href: "https://x.com/docs?ref=note",
      target: null,
    });
    let state = createState(
      schema.node("doc", null, [
        schema.node("paragraph", null, [
          schema.text("x.com", [link]),
          schema.text(" note"),
        ]),
      ]),
    );

    state = state.applyTransaction(
      state.tr.insertText("!", state.doc.content.size - 1),
    ).state;

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "x.com",
              marks: [
                {
                  type: "link",
                  attrs: {
                    href: "https://x.com/docs?ref=note",
                    target: null,
                  },
                },
              ],
            },
            { type: "text", text: " note!" },
          ],
        },
      ],
    });
  });

  it("preserves link attrs when adjacent path text is typed", () => {
    const link = schema.marks.link.create({
      href: "https://x.com",
      target: "_blank",
    });
    let state = createState(
      schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("x.com", [link])]),
      ]),
    );

    state = state.applyTransaction(
      state.tr.insertText("/docs", state.doc.content.size - 1),
    ).state;

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "x.com/docs",
              marks: [
                {
                  type: "link",
                  attrs: {
                    href: "https://x.com/docs",
                    target: "_blank",
                  },
                },
              ],
            },
          ],
        },
      ],
    });
  });
});

function insertText(text: string) {
  const doc = schema.node("doc", null, [schema.node("paragraph")]);
  const state = createState(doc);

  return state.applyTransaction(state.tr.insertText(text, 1)).state;
}

function createState(doc: ProseMirrorNode) {
  return EditorState.create({
    schema,
    doc,
    plugins: [autolinkPlugin(), linkBoundaryGuardPlugin()],
  });
}
