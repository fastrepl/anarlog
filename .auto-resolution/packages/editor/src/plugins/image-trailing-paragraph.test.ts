import { EditorState } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import { imageTrailingParagraphPlugin } from "./image-trailing-paragraph";

describe("imageTrailingParagraphPlugin", () => {
  it("adds a paragraph after an inserted image", () => {
    const image = schema.nodes.image.create({ src: "image.png" });
    let state = createState([schema.node("paragraph")]);

    state = state.applyTransaction(state.tr.replaceWith(0, 2, image)).state;

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            src: "image.png",
            alt: null,
            title: null,
            attachmentId: null,
            sharedAttachmentId: null,
            editorWidth: 80,
          },
        },
        { type: "paragraph" },
      ],
    });
  });

  it("restores a trailing paragraph when the one after an image is removed", () => {
    const image = schema.nodes.image.create({ src: "image.png" });
    let state = createState([image, schema.node("paragraph")]);

    state = state.applyTransaction(
      state.tr.delete(image.nodeSize, state.doc.content.size),
    ).state;

    expect(state.doc.childCount).toBe(2);
    expect(state.doc.child(0).type).toBe(schema.nodes.image);
    expect(state.doc.child(1).type).toBe(schema.nodes.paragraph);
  });

  it("adds paragraphs after multiple pasted images without shifting positions", () => {
    const firstImage = schema.nodes.image.create({ src: "first.png" });
    const secondImage = schema.nodes.image.create({ src: "second.png" });
    let state = createState([schema.node("paragraph")]);

    state = state.applyTransaction(
      state.tr.replaceWith(0, 2, [firstImage, secondImage]),
    ).state;

    expect(state.doc.childCount).toBe(4);
    expect(state.doc.child(0).type).toBe(schema.nodes.image);
    expect(state.doc.child(1).type).toBe(schema.nodes.paragraph);
    expect(state.doc.child(2).type).toBe(schema.nodes.image);
    expect(state.doc.child(3).type).toBe(schema.nodes.paragraph);
  });

  it("does not change the document for normal text edits", () => {
    let state = createState([
      schema.node("paragraph", null, [schema.text("hello")]),
      schema.node("paragraph", null, [schema.text("world")]),
    ]);

    state = state.applyTransaction(state.tr.insertText("!", 6)).state;

    expect(state.doc.textContent).toBe("hello!world");
    expect(state.doc.childCount).toBe(2);
  });

  it("handles edits at document boundaries", () => {
    let state = createState([schema.node("paragraph")]);

    state = state.applyTransaction(state.tr.insertText("x", 1)).state;

    expect(state.doc.textContent).toBe("x");
  });
});

function createState(content: Parameters<typeof schema.node>[2]) {
  return EditorState.create({
    schema,
    doc: schema.node("doc", null, content),
    plugins: [imageTrailingParagraphPlugin()],
  });
}
