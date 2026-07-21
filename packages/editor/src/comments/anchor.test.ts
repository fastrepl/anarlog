import { type Node as PMNode } from "prosemirror-model";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import {
  captureCommentAnchor,
  MAX_ANCHOR_QUOTE_LENGTH,
  resolveCommentAnchor,
  resolveCommentAnchors,
} from "./anchor";
import {
  ANCHOR_BLOCK_SEPARATOR,
  ANCHOR_LEAF_TEXT,
  buildDocTextIndex,
} from "./anchor-text";

const paragraph = (text: string) =>
  schema.node("paragraph", null, text ? [schema.text(text)] : []);

const doc = (...children: PMNode[]) => schema.node("doc", null, children);

const fixture = () =>
  doc(
    schema.node("heading", { level: 1 }, [schema.text("Fixture title")]),
    paragraph("The quick brown fox jumps over the lazy dog."),
    schema.node("bulletList", null, [
      schema.node("listItem", null, [paragraph("First bullet point")]),
      schema.node("listItem", null, [paragraph("Second bullet point")]),
    ]),
    paragraph("A closing paragraph with unique words."),
  );

describe("buildDocTextIndex", () => {
  it("matches ProseMirror textBetween over mixed fixtures", () => {
    const fixtures = [
      fixture(),
      doc(paragraph("only one paragraph")),
      doc(
        paragraph("before image"),
        schema.node("paragraph", null, [
          schema.text("with "),
          schema.node("hardBreak"),
          schema.text(" break"),
        ]),
        paragraph("after"),
      ),
      doc(
        schema.node("blockquote", null, [paragraph("quoted block")]),
        paragraph("plain"),
      ),
    ];
    for (const document of fixtures) {
      expect(buildDocTextIndex(document).text).toBe(
        document.textBetween(
          0,
          document.content.size,
          ANCHOR_BLOCK_SEPARATOR,
          ANCHOR_LEAF_TEXT,
        ),
      );
    }
  });

  it("round-trips offsets and positions inside a text block", () => {
    const document = doc(paragraph("hello world"));
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("world");
    const pos = index.posAt(offset);
    expect(index.indexAt(pos)).toBe(offset);
    expect(document.textBetween(pos, index.endPosAt(offset + 5))).toBe("world");
  });
});

describe("captureCommentAnchor", () => {
  it("captures the quote with prefix and suffix context", () => {
    const document = fixture();
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("brown fox");
    const from = index.posAt(offset);
    const to = index.endPosAt(offset + "brown fox".length);

    const anchor = captureCommentAnchor(document, from, to, 3);
    expect(anchor).not.toBeNull();
    expect(anchor?.quoteExact).toBe("brown fox");
    expect(anchor?.quotePrefix.endsWith("The quick ")).toBe(true);
    expect(anchor?.quoteSuffix.startsWith(" jumps over")).toBe(true);
    expect(anchor?.fromHint).toBe(from);
    expect(anchor?.toHint).toBe(to);
    expect(anchor?.snapshotRevision).toBe(3);
  });

  it("captures across block boundaries with the separator", () => {
    const document = fixture();
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("First bullet");
    const end = index.text.indexOf("Second bullet") + "Second".length;
    const anchor = captureCommentAnchor(
      document,
      index.posAt(offset),
      index.endPosAt(end),
      1,
    );
    expect(anchor?.quoteExact).toContain(ANCHOR_BLOCK_SEPARATOR);
    expect(anchor?.quoteExact.startsWith("First bullet")).toBe(true);
  });

  it("trims boundary separators so quotes survive neighbor-block changes", () => {
    const document = doc(paragraph("first block"), paragraph("second block"));
    const index = buildDocTextIndex(document);
    const separator = index.text.indexOf(ANCHOR_BLOCK_SEPARATOR);
    // Selection starts at the end of block one, before the separator.
    const from = index.endPosAt(separator);
    const to = index.endPosAt(index.text.indexOf("second") + "second".length);

    const anchor = captureCommentAnchor(document, from, to, 1)!;
    expect(anchor.quoteExact).toBe("second");
    expect(anchor.quoteExact.startsWith(ANCHOR_BLOCK_SEPARATOR)).toBe(false);

    expect(resolveCommentAnchor(document, anchor, 1)).not.toBeNull();

    const withoutFirstBlock = doc(paragraph("second block"));
    const resolved = resolveCommentAnchor(withoutFirstBlock, anchor, 2);
    expect(resolved).not.toBeNull();
    expect(withoutFirstBlock.textBetween(resolved!.from, resolved!.to)).toBe(
      "second",
    );
  });

  it("keeps real newlines inside code blocks while trimming separators", () => {
    const document = doc(
      paragraph("before"),
      schema.node("codeBlock", null, [schema.text("line one\nline two")]),
    );
    const index = buildDocTextIndex(document);
    const start = index.text.indexOf("one");
    // Selection ends on the literal newline inside the code block.
    const end = index.text.indexOf("\nline two", start) + 1;
    const anchor = captureCommentAnchor(
      document,
      index.posAt(start),
      index.endPosAt(end),
      1,
    )!;
    expect(anchor.quoteExact).toBe("one\n");

    const resolved = resolveCommentAnchor(document, anchor, 1);
    expect(resolved).not.toBeNull();
    expect(document.textBetween(resolved!.from, resolved!.to)).toBe("one\n");
  });

  it("returns null for empty or oversized selections", () => {
    const document = fixture();
    expect(captureCommentAnchor(document, 5, 5, 1)).toBeNull();

    const longText = "y".repeat(MAX_ANCHOR_QUOTE_LENGTH + 10);
    const longDoc = doc(paragraph(longText));
    expect(captureCommentAnchor(longDoc, 1, 1 + longText.length, 1)).toBeNull();
  });
});

describe("resolveCommentAnchor", () => {
  it("uses verified hints at the matching revision", () => {
    const document = fixture();
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("lazy dog");
    const from = index.posAt(offset);
    const to = index.endPosAt(offset + "lazy dog".length);
    const anchor = captureCommentAnchor(document, from, to, 5)!;

    expect(resolveCommentAnchor(document, anchor, 5)).toEqual({ from, to });
  });

  it("rejects stale hints and re-resolves by quote", () => {
    const document = fixture();
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("closing paragraph");
    const anchor = captureCommentAnchor(
      document,
      index.posAt(offset),
      index.endPosAt(offset + "closing paragraph".length),
      5,
    )!;

    const grown = doc(
      paragraph("A brand new opening paragraph."),
      ...document.children,
    );
    const resolved = resolveCommentAnchor(grown, anchor, 6);
    expect(resolved).not.toBeNull();
    expect(grown.textBetween(resolved!.from, resolved!.to)).toBe(
      "closing paragraph",
    );
  });

  it("disambiguates duplicate quotes by context and refuses ties", () => {
    const duplicated = doc(
      paragraph("alpha target beta"),
      paragraph("gamma target delta"),
    );
    const index = buildDocTextIndex(duplicated);
    const second = index.text.lastIndexOf("target");
    const anchor = captureCommentAnchor(
      duplicated,
      index.posAt(second),
      index.endPosAt(second + "target".length),
      1,
    )!;

    const resolved = resolveCommentAnchor(duplicated, anchor, 2);
    expect(resolved).not.toBeNull();
    expect(index.indexAt(resolved!.from)).toBe(second);

    const tied = doc(
      paragraph("alpha target beta"),
      paragraph("alpha target beta"),
    );
    const tieAnchor = {
      ...anchor,
      quotePrefix: "alpha ",
      quoteSuffix: " beta",
    };
    expect(resolveCommentAnchor(tied, tieAnchor, 2)).toBeNull();
  });

  it("returns null when the quoted text is gone", () => {
    const document = fixture();
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("unique words");
    const anchor = captureCommentAnchor(
      document,
      index.posAt(offset),
      index.endPosAt(offset + "unique words".length),
      1,
    )!;

    const rewritten = doc(paragraph("Entirely different content now."));
    expect(resolveCommentAnchor(rewritten, anchor, 2)).toBeNull();
  });

  it("resolves batches with one shared index", () => {
    const document = fixture();
    const index = buildDocTextIndex(document);
    const offset = index.text.indexOf("Fixture title");
    const anchor = captureCommentAnchor(
      document,
      index.posAt(offset),
      index.endPosAt(offset + "Fixture title".length),
      1,
    )!;

    const resolved = resolveCommentAnchors(
      document,
      [
        { commentId: "a", anchor },
        { commentId: "b", anchor: null },
        {
          commentId: "c",
          anchor: { ...anchor, quoteExact: "not in the document" },
        },
      ],
      1,
    );
    expect(resolved[0].range).not.toBeNull();
    expect(resolved[1].range).toBeNull();
    expect(resolved[2].range).toBeNull();
  });
});
