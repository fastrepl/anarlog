import {
  MarkdownSerializer,
  type MarkdownSerializerState,
} from "prosemirror-markdown";
import { Node as PMNode } from "prosemirror-model";

import { serializeMarkdownImage } from "../image-markdown";
import { markdownSchema } from "./schema";

// ---------------------------------------------------------------------------

function backticksFor(node: PMNode, side: number): string {
  const ticks = /`+/g;
  let m;
  let len = 0;
  if (node.isText) {
    while ((m = ticks.exec(node.text!))) {
      len = Math.max(len, m[0].length);
    }
  }
  let result = len > 0 && side > 0 ? " `" : "`";
  for (let i = 0; i < len; i++) result += "`";
  if (len > 0 && side < 0) result += " ";
  return result;
}

function escapeTableCell(markdown: string): string {
  const breakPlaceholder = "\u0000TABLE_CELL_BREAK\u0000";

  return markdown
    .replace(/\\\n/g, breakPlaceholder)
    .replace(/\n+/g, breakPlaceholder)
    .replace(/\|/g, "\\|")
    .replace(/<br\s*\/?>/gi, "\\$&")
    .split(breakPlaceholder)
    .join("<br>")
    .trim();
}

function tableCellToMarkdown(cell: PMNode): string {
  const parts: string[] = [];

  cell.forEach((child) => {
    const doc = markdownSchema.node("doc", null, [child]);
    parts.push(getSerializer().serialize(doc));
  });

  return escapeTableCell(parts.join("\n"));
}

function tableCellSpan(cell: PMNode, attr: "colspan" | "rowspan"): number {
  const span = Number(cell.attrs[attr] ?? 1);
  return Number.isInteger(span) && span > 0 ? span : 1;
}

function tableCellsToMarkdown(cells: string[]): string {
  return `| ${cells.join(" | ")} |`;
}

function tableRowsToMarkdown(rows: PMNode[]): {
  rows: string[];
  columnCount: number;
} {
  const pendingRowspans: number[] = [];
  const renderedRows: string[][] = [];
  let columnCount = 0;

  for (const row of rows) {
    const cells: string[] = [];
    let columnIndex = 0;

    const addPendingRowspans = () => {
      while ((pendingRowspans[columnIndex] ?? 0) > 0) {
        cells.push("");
        pendingRowspans[columnIndex] -= 1;
        columnIndex += 1;
      }
    };

    row.forEach((cell) => {
      addPendingRowspans();

      const startColumn = columnIndex;
      const colspan = tableCellSpan(cell, "colspan");
      const rowspan = tableCellSpan(cell, "rowspan");

      cells.push(tableCellToMarkdown(cell));
      columnIndex += 1;

      for (let i = 1; i < colspan; i++) {
        cells.push("");
        columnIndex += 1;
      }

      if (rowspan > 1) {
        for (let i = 0; i < colspan; i++) {
          pendingRowspans[startColumn + i] = Math.max(
            pendingRowspans[startColumn + i] ?? 0,
            rowspan - 1,
          );
        }
      }
    });

    addPendingRowspans();
    columnCount = Math.max(columnCount, cells.length);
    renderedRows.push(cells);
  }

  columnCount = Math.max(columnCount, 1);

  return {
    rows: renderedRows.map((cells) => {
      while (cells.length < columnCount) {
        cells.push("");
      }

      return tableCellsToMarkdown(cells);
    }),
    columnCount,
  };
}

function tableDelimiterRow(columnCount: number): string {
  return `| ${Array.from({ length: columnCount }, () => "---").join(" | ")} |`;
}

let _serializer: MarkdownSerializer | null = null;

export function getSerializer(): MarkdownSerializer {
  if (_serializer) return _serializer;

  _serializer = new MarkdownSerializer(
    {
      blockquote(state, node) {
        state.wrapBlock("> ", null, node, () => state.renderContent(node));
      },

      codeBlock(state, node) {
        const backticks = node.textContent.match(/`{3,}/gm);
        const fence = backticks ? backticks.sort().slice(-1)[0] + "`" : "```";
        state.write(fence + (node.attrs.language || "") + "\n");
        state.text(node.textContent, false);
        state.write("\n");
        state.write(fence);
        state.closeBlock(node);
      },

      heading(state, node) {
        state.write(state.repeat("#", node.attrs.level) + " ");
        state.renderInline(node);
        state.closeBlock(node);
      },

      horizontalRule(state, node) {
        state.write("---");
        state.closeBlock(node);
      },

      bulletList(state, node) {
        state.renderList(node, "  ", () => "- ");
      },

      orderedList(state, node) {
        const start = node.attrs.start || 1;
        const maxW = String(start + node.childCount - 1).length;
        const space = state.repeat(" ", maxW + 2);
        state.renderList(node, space, (i) => {
          const nStr = String(start + i);
          return state.repeat(" ", maxW - nStr.length) + nStr + ". ";
        });
      },

      listItem(state, node) {
        state.renderContent(node);
      },

      table(state, node) {
        const rows: PMNode[] = [];
        node.forEach((row) => rows.push(row));
        if (rows.length === 0) {
          state.closeBlock(node);
          return;
        }

        const table = tableRowsToMarkdown(rows);

        state.write(table.rows[0]);
        state.write("\n");
        state.write(tableDelimiterRow(table.columnCount));

        for (let i = 1; i < table.rows.length; i++) {
          state.write("\n");
          state.write(table.rows[i]);
        }

        state.closeBlock(node);
      },

      tableRow() {},

      tableCell() {},

      tableHeader() {},

      taskList(state, node) {
        state.renderList(node, "  ", () => "- ");
      },

      taskItem(state, node) {
        const checkbox = node.attrs.checked ? "[x] " : "[ ] ";
        state.write(checkbox);
        state.renderContent(node);
      },

      paragraph(state, node) {
        if (node.childCount === 0) {
          // Force flushClose of the previous block before marking ourselves
          // closed — produces an extra blank line per empty paragraph.
          state.write("");
        } else {
          state.renderInline(node);
        }
        state.closeBlock(node);
      },

      image(state, node) {
        state.write(
          serializeMarkdownImage({
            src: node.attrs.src,
            alt: node.attrs.alt,
            title: node.attrs.title,
            editorWidth: node.attrs.editorWidth,
            escapeAlt: (value) => state.esc(value),
          }),
        );
      },

      hardBreak(state, node, parent, index) {
        for (let i = index + 1; i < parent.childCount; i++) {
          if (parent.child(i).type !== node.type) {
            state.write("\\\n");
            return;
          }
        }
      },

      text(state, node) {
        state.text(node.text!, true);
      },

      clip(state, node) {
        const src = (node.attrs.src || "").replace(/"/g, "&quot;");
        state.write(`<Clip src="${src}" />`);
        state.closeBlock(node);
      },

      fileAttachment(state, node) {
        const name = node.attrs.name || "file";
        const attachmentId = node.attrs.attachmentId || "";
        const source =
          /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(
            attachmentId,
          )
            ? `attachment://${attachmentId}`
            : node.attrs.src || "";
        const src = source.replace(/\(/g, "%28").replace(/\)/g, "%29");
        state.write(`[${name}](${src})`);
        state.closeBlock(node);
      },

      "mention-@"(state, node) {
        const id = node.attrs.id ?? "";
        const type = node.attrs.type ?? "";
        const label = node.attrs.label ?? "";
        state.write(
          `<mention data-id="${id}" data-type="${type}" data-label="${label}"></mention>`,
        );
      },
    },
    {
      bold: {
        open: "**",
        close: "**",
        mixable: true,
        expelEnclosingWhitespace: true,
      },
      italic: {
        open: "*",
        close: "*",
        mixable: true,
        expelEnclosingWhitespace: true,
      },
      underline: { open: "<u>", close: "</u>" },
      strike: {
        open: "~~",
        close: "~~",
        mixable: true,
        expelEnclosingWhitespace: true,
      },
      code: {
        open(_state: MarkdownSerializerState, _mark, parent, index) {
          return backticksFor(parent.child(index), -1);
        },
        close(_state: MarkdownSerializerState, _mark, parent, index) {
          return backticksFor(parent.child(index - 1), 1);
        },
        escape: false,
      },
      link: {
        open: "[",
        close(_state: MarkdownSerializerState, mark) {
          const href = mark.attrs.href
            ? mark.attrs.href.replace(/[()]/g, "\\$&")
            : "";
          return `](${href})`;
        },
        mixable: true,
      },
      highlight: { open: "==", close: "==" },
    },
    {
      hardBreakNodeName: "hardBreak",
      strict: false,
    },
  );

  return _serializer;
}

// ---------------------------------------------------------------------------
// Public API
