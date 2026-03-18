import { describe, expect, test } from "vitest";

import {
  rewriteHtmlImageSources,
  rewriteMarkdownImageSources,
} from "./clipboard";

describe("rewriteMarkdownImageSources", () => {
  test("replaces image sources and preserves titles", async () => {
    const markdown =
      'Before\n\n![image 1](asset://localhost/%2Ftmp%2Fone.png "Title")\n\nAfter';

    const result = await rewriteMarkdownImageSources(markdown, async (src) =>
      src.includes("one.png") ? "data:image/png;base64,AAA" : null,
    );

    expect(result).toContain('![image 1](data:image/png;base64,AAA "Title")');
    expect(result).toContain("Before");
    expect(result).toContain("After");
  });

  test("leaves non-resolved images unchanged", async () => {
    const markdown = "![image 1](https://example.com/one.png)";

    const result = await rewriteMarkdownImageSources(
      markdown,
      async () => null,
    );

    expect(result).toBe(markdown);
  });
});

describe("rewriteHtmlImageSources", () => {
  test("replaces image sources and adds width styles", async () => {
    const html =
      '<p>Before</p><img src="asset://localhost/%2Ftmp%2Fone.png" alt="image 1" data-editor-width="42"><p>After</p>';

    const result = await rewriteHtmlImageSources(html, async (src) =>
      src.includes("one.png") ? "data:image/png;base64,AAA" : null,
    );

    expect(result).toContain('src="data:image/png;base64,AAA"');
    expect(result).toContain('style="width: 42%;"');
    expect(result).toContain("<p>Before</p>");
    expect(result).toContain("<p>After</p>");
  });

  test("merges width styles with existing inline styles", async () => {
    const html =
      '<img src="asset://localhost/%2Ftmp%2Fone.png" style="display:block;" data-editor-width="50">';

    const result = await rewriteHtmlImageSources(
      html,
      async () => "data:image/png;base64,AAA",
    );

    expect(result).toContain('src="data:image/png;base64,AAA"');
    expect(result).toContain('style="display:block; width: 50%;"');
  });
});
