import { Fragment, Slice } from "prosemirror-model";
import { describe, expect, test } from "vitest";

import { schema } from "../note/schema";
import { serializeClipboardText } from "./clipboard";

describe("serializeClipboardText", () => {
  test("serializes selected images as markdown", () => {
    const slice = new Slice(
      Fragment.fromArray([
        schema.nodes.paragraph.create(null, schema.text("Before")),
        schema.nodes.image.create({
          src: "asset://localhost/session/image.png",
          alt: "diagram",
          title: "Architecture",
          editorWidth: 64,
        }),
        schema.nodes.paragraph.create(null, schema.text("After")),
      ]),
      0,
      0,
    );

    expect(serializeClipboardText(slice)).toBe(
      'Before\n\n![diagram](asset://localhost/session/image.png "char-editor-width=64|Architecture")\n\nAfter',
    );
  });

  test("escapes markdown image fields", () => {
    const slice = new Slice(
      Fragment.from(
        schema.nodes.image.create({
          src: "https://example.com/screenshots/a(b).png",
          alt: "diagram] detail",
          title: 'Quote "title"',
          editorWidth: 42,
        }),
      ),
      0,
      0,
    );

    expect(serializeClipboardText(slice)).toBe(
      '![diagram\\] detail](https://example.com/screenshots/a\\(b\\).png "char-editor-width=42|Quote \\"title\\"")',
    );
  });
});
