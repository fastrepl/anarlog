import { MarkdownManager } from "@tiptap/markdown";
import { describe, expect, test } from "vitest";

import { ClipNode, parseYouTubeEmbedSnippet, parseYouTubeUrl } from "./clip";
import { getExtensions } from "./extensions";

function getClipMarkdownManager() {
  return new MarkdownManager({
    extensions: [...getExtensions(), ClipNode],
  });
}

describe("clip markdown support", () => {
  test("normalizes YouTube URLs to embed URLs", () => {
    expect(
      parseYouTubeUrl("https://www.youtube.com/watch?v=abc123&t=45s"),
    ).toEqual({
      embedUrl: "https://www.youtube.com/embed/abc123?start=45",
    });

    expect(
      parseYouTubeUrl("https://www.youtube-nocookie.com/embed/xyz789"),
    ).toEqual({
      embedUrl: "https://www.youtube.com/embed/xyz789",
    });
  });

  test("parses iframe embed snippets", () => {
    expect(
      parseYouTubeEmbedSnippet(
        '<iframe src="https://www.youtube.com/embed/abc123?start=30"></iframe>',
      ),
    ).toEqual({
      embedUrl: "https://www.youtube.com/embed/abc123?start=30",
    });
  });

  test("parses Clip MDX blocks into clip nodes", () => {
    const markdown =
      '<Clip src="https://www.youtube.com/watch?v=abc123&t=45s" />';

    expect(getClipMarkdownManager().parse(markdown)).toEqual({
      type: "doc",
      content: [
        {
          type: "clip",
          attrs: {
            src: "https://www.youtube.com/embed/abc123?start=45",
          },
        },
      ],
    });
  });

  test("parses iframe blocks into clip nodes", () => {
    const markdown =
      '<iframe src="https://www.youtube.com/embed/abc123"></iframe>';

    expect(getClipMarkdownManager().parse(markdown)).toEqual({
      type: "doc",
      content: [
        {
          type: "clip",
          attrs: {
            src: "https://www.youtube.com/embed/abc123",
          },
        },
      ],
    });
  });

  test("serializes clip nodes back to Clip MDX", () => {
    const markdown = getClipMarkdownManager().serialize({
      type: "doc",
      content: [
        {
          type: "clip",
          attrs: {
            src: "https://www.youtube.com/embed/abc123",
          },
        },
      ],
    });

    expect(markdown).toBe(
      '<Clip src="https://www.youtube.com/embed/abc123" />',
    );
  });
});
