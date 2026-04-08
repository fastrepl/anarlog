// @vitest-environment jsdom

import { Editor, type JSONContent } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { afterEach, describe, expect, it } from "vitest";

import { Hashtag, findHashtags } from "./hashtag";
import { md2json } from "./utils";

const editors: Editor[] = [];

function createEditor(content: JSONContent) {
  const editor = new Editor({
    extensions: [StarterKit, Hashtag],
    content,
  });

  editors.push(editor);

  return editor;
}

function getDecoratedHashtags(editor: Editor) {
  const decorationPlugin = editor.state.plugins.find((plugin) =>
    plugin.key.startsWith("hashtagDecoration"),
  );

  expect(decorationPlugin).toBeDefined();

  return (
    decorationPlugin?.props
      .decorations?.(editor.state)
      ?.find()
      .map((decoration) =>
        editor.state.doc.textBetween(decoration.from, decoration.to),
      ) ?? []
  );
}

afterEach(() => {
  while (editors.length > 0) {
    editors.pop()?.destroy();
  }
});

describe("findHashtags", () => {
  it("extracts regular hashtags", () => {
    expect(findHashtags("#alpha #beta").map((match) => match.tag)).toEqual([
      "alpha",
      "beta",
    ]);
  });

  it("ignores url fragments", () => {
    expect(
      findHashtags("https://web4.ai/#free #valid").map((match) => match.tag),
    ).toEqual(["valid"]);
  });

  it("ignores www url fragments", () => {
    expect(
      findHashtags("www.web4.ai/#free #valid").map((match) => match.tag),
    ).toEqual(["valid"]);
  });
});

describe("Hashtag decorations", () => {
  it("does not highlight hashtags inside code blocks", () => {
    const editor = createEditor({
      type: "doc",
      content: [
        {
          type: "codeBlock",
          content: [{ type: "text", text: "#inside" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "outside #visible" }],
        },
      ],
    });

    expect(getDecoratedHashtags(editor)).toEqual(["#visible"]);
  });

  it("does not highlight hashtags inside inline code", () => {
    const editor = createEditor({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            { type: "text", text: "Use " },
            {
              type: "text",
              text: "#inline",
              marks: [{ type: "code" }],
            },
            { type: "text", text: " and #visible" },
          ],
        },
      ],
    });

    expect(getDecoratedHashtags(editor)).toEqual(["#visible"]);
  });

  it("does not highlight hashtags inside inline code parsed from markdown", () => {
    const editor = createEditor(
      md2json("Use `## Meeting history` and keep #visible highlighted"),
    );

    expect(getDecoratedHashtags(editor)).toEqual(["#visible"]);
  });
});
