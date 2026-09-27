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
  let list: Extract<MarkdownBlock, { type: "list" }> | null = null;
  let indents: number[] = [];

  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    blocks.push({ type: "paragraph", spans: parseInline(paragraph.join(" ")) });
    paragraph = [];
  };

  for (const rawLine of markdown.replace(/\r\n?/g, "\n").split("\n")) {
    const line = rawLine.trimEnd();
    if (!line.trim() || RULE.test(line)) {
      flushParagraph();
      list = null;
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      flushParagraph();
      list = null;
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
      if (!list) {
        list = { type: "list", items: [] };
        blocks.push(list);
        indents = [];
      }
      const indent = item[1].replace(/\t/g, "    ").length;
      while (indents.length > 0 && indents[indents.length - 1] > indent) {
        indents.pop();
      }
      if (indents.length === 0 || indents[indents.length - 1] < indent) {
        indents.push(indent);
      }
      const depth = indents.length - 1;
      const sibling = [...list.items]
        .reverse()
        .find((previous) => previous.depth <= depth);
      const number = !ordered
        ? undefined
        : sibling?.depth === depth && sibling.number !== undefined
          ? sibling.number + 1
          : Number(ordered[2]);
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
        number,
        checked,
      };
      list.items.push(entry);
      continue;
    }

    list = null;
    paragraph.push(line.trim());
  }

  flushParagraph();
  return blocks;
}
