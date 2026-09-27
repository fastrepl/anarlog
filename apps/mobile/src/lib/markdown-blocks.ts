export type InlineSpan = {
  text: string;
  bold?: boolean;
  italic?: boolean;
  code?: boolean;
};

export type MarkdownBlock =
  | { type: "heading"; level: number; spans: InlineSpan[] }
  | { type: "paragraph"; spans: InlineSpan[] }
  | {
      type: "list";
      items: Array<{
        spans: InlineSpan[];
        depth: number;
        number?: number;
        checked?: boolean;
      }>;
    };

const HEADING = /^(#{1,6})\s+(.*)$/;
const BULLET = /^(\s*)[-*+]\s+(.*)$/;
const ORDERED = /^(\s*)(\d+)[.)]\s+(.*)$/;
const CHECKBOX = /^\[([ xX])\]\s+(.*)$/;
const RULE = /^\s*([-*_])(?:\s*\1){2,}\s*$/;

const INLINE =
  /(\*\*[^*\n]+?\*\*|__[^_\n]+?__|`[^`\n]+`|\*[^*\n]+?\*|_[^_\n]+?_)/;

export function parseInline(text: string): InlineSpan[] {
  const spans: InlineSpan[] = [];
  for (const part of text.split(INLINE)) {
    if (!part) continue;
    if (part.length >= 4 && (part.startsWith("**") || part.startsWith("__"))) {
      spans.push({ text: part.slice(2, -2), bold: true });
    } else if (part.length >= 3 && part.startsWith("`")) {
      spans.push({ text: part.slice(1, -1), code: true });
    } else if (
      part.length >= 3 &&
      (part.startsWith("*") || part.startsWith("_"))
    ) {
      spans.push({ text: part.slice(1, -1), italic: true });
    } else {
      spans.push({ text: part });
    }
  }
  return spans;
}

export function parseMarkdownBlocks(markdown: string): MarkdownBlock[] {
  const blocks: MarkdownBlock[] = [];
  let paragraph: string[] = [];

  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    blocks.push({ type: "paragraph", spans: parseInline(paragraph.join(" ")) });
    paragraph = [];
  };

  for (const rawLine of markdown.replace(/\r\n?/g, "\n").split("\n")) {
    const line = rawLine.trimEnd();
    if (!line.trim() || RULE.test(line)) {
      flushParagraph();
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      flushParagraph();
      blocks.push({
        type: "heading",
        level: heading[1].length,
        spans: parseInline(heading[2].trim()),
      });
      continue;
    }

    const bullet = BULLET.exec(line);
    const ordered = bullet ? null : ORDERED.exec(line);
    const item = bullet ?? ordered;
    if (item) {
      flushParagraph();
      const depth = Math.floor(item[1].replace(/\t/g, "  ").length / 2);
      let content = ordered ? ordered[3] : item[2];
      let checked: boolean | undefined;
      const checkbox = bullet ? CHECKBOX.exec(content) : null;
      if (checkbox) {
        checked = checkbox[1] !== " ";
        content = checkbox[2];
      }
      const entry = {
        spans: parseInline(content),
        depth,
        number: ordered ? Number(ordered[2]) : undefined,
        checked,
      };
      const previous = blocks[blocks.length - 1];
      if (previous?.type === "list") {
        previous.items.push(entry);
      } else {
        blocks.push({ type: "list", items: [entry] });
      }
      continue;
    }

    paragraph.push(line.trim());
  }

  flushParagraph();
  return blocks;
}
