import MarkdownIt from "markdown-it";
import type StateBlock from "markdown-it/lib/rules_block/state_block.mjs";
import type StateInline from "markdown-it/lib/rules_inline/state_inline.mjs";
import type Token from "markdown-it/lib/token.mjs";
import { MarkdownParser } from "prosemirror-markdown";

import {
  DEFAULT_EDITOR_WIDTH,
  parseImageTitleMetadata,
} from "../image-markdown";
import { markdownSchema } from "./schema";

// ---------------------------------------------------------------------------

function strikethroughPlugin(md: MarkdownIt) {
  md.inline.ruler.before(
    "emphasis",
    "strikethrough",
    (state: StateInline, silent: boolean) => {
      const start = state.pos;
      const marker = state.src.charCodeAt(start);
      if (marker !== 0x7e /* ~ */) return false;
      if (state.src.charCodeAt(start + 1) !== 0x7e) return false;

      const match = state.src.slice(start).match(/^~~([\s\S]+?)~~/);
      if (!match) return false;

      if (!silent) {
        const token = state.push("s_open", "s", 1);
        token.markup = "~~";

        const content = state.push("text", "", 0);
        content.content = match[1];

        const close = state.push("s_close", "s", -1);
        close.markup = "~~";
      }

      state.pos += match[0].length;
      return true;
    },
  );
}

function underlinePlugin(md: MarkdownIt) {
  md.inline.ruler.before(
    "emphasis",
    "underline",
    (state: StateInline, silent: boolean) => {
      const start = state.pos;
      const src = state.src.slice(start);

      // ++text++ syntax
      if (
        state.src.charCodeAt(start) === 0x2b /* + */ &&
        state.src.charCodeAt(start + 1) === 0x2b
      ) {
        const match = src.match(/^\+\+([\s\S]+?)\+\+/);
        if (match) {
          if (!silent) {
            const open = state.push("underline_open", "u", 1);
            open.markup = "++";
            const text = state.push("text", "", 0);
            text.content = match[1];
            const close = state.push("underline_close", "u", -1);
            close.markup = "++";
          }
          state.pos += match[0].length;
          return true;
        }
      }

      // <u>text</u> syntax
      if (state.src.charCodeAt(start) === 0x3c /* < */) {
        const match = src.match(/^<u>([\s\S]+?)<\/u>/);
        if (match) {
          if (!silent) {
            const open = state.push("underline_open", "u", 1);
            open.markup = "<u>";
            const text = state.push("text", "", 0);
            text.content = match[1];
            const close = state.push("underline_close", "u", -1);
            close.markup = "</u>";
          }
          state.pos += match[0].length;
          return true;
        }
      }

      return false;
    },
  );
}

function highlightPlugin(md: MarkdownIt) {
  md.inline.ruler.before(
    "emphasis",
    "highlight",
    (state: StateInline, silent: boolean) => {
      const start = state.pos;
      if (
        state.src.charCodeAt(start) !== 0x3d /* = */ ||
        state.src.charCodeAt(start + 1) !== 0x3d
      ) {
        return false;
      }

      const match = state.src.slice(start).match(/^==([\s\S]+?)==/);
      if (!match) return false;

      if (!silent) {
        const open = state.push("highlight_open", "mark", 1);
        open.markup = "==";
        const text = state.push("text", "", 0);
        text.content = match[1];
        const close = state.push("highlight_close", "mark", -1);
        close.markup = "==";
      }

      state.pos += match[0].length;
      return true;
    },
  );
}

function hardBreakTagPlugin(md: MarkdownIt) {
  md.inline.ruler.before(
    "html_inline",
    "hardbreak_tag",
    (state: StateInline, silent: boolean) => {
      const match = state.src.slice(state.pos).match(/^<br\s*\/?>/i);
      if (!match) return false;

      if (!silent) {
        const token = state.push("hardbreak", "br", 0);
        token.markup = match[0];
      }

      state.pos += match[0].length;
      return true;
    },
  );
}

function taskListPlugin(md: MarkdownIt) {
  md.core.ruler.after("inline", "task_lists", (state) => {
    const tokens = state.tokens;
    for (let i = 0; i < tokens.length; i++) {
      if (tokens[i].type !== "bullet_list_open") continue;

      let hasTask = false;
      let j = i + 1;
      while (j < tokens.length && tokens[j].type !== "bullet_list_close") {
        if (tokens[j].type === "list_item_open") {
          const inlineIdx = findInlineToken(tokens, j);
          if (
            inlineIdx !== -1 &&
            isTaskItemContent(tokens[inlineIdx].content)
          ) {
            hasTask = true;
            break;
          }
        }
        j++;
      }

      if (!hasTask) continue;

      const closeIdx = findMatchingClose(
        tokens,
        i,
        "bullet_list_open",
        "bullet_list_close",
      );
      if (closeIdx === -1) continue;

      tokens[i].type = "task_list_open";
      tokens[i].tag = "ul";
      tokens[closeIdx].type = "task_list_close";
      tokens[closeIdx].tag = "ul";

      for (let k = i + 1; k < closeIdx; k++) {
        if (tokens[k].type === "list_item_open") {
          const inlineIdx = findInlineToken(tokens, k);
          if (inlineIdx !== -1) {
            const content = tokens[inlineIdx].content;
            const taskMatch = content.match(/^\[([ xX])\]\s*/);
            if (taskMatch) {
              const checked = taskMatch[1].toLowerCase() === "x";
              tokens[k].type = "task_item_open";
              tokens[k].attrSet("checked", checked ? "true" : "false");

              tokens[inlineIdx].content = content.slice(taskMatch[0].length);
              if (tokens[inlineIdx].children) {
                stripTaskPrefix(
                  tokens[inlineIdx].children!,
                  taskMatch[0].length,
                );
              }
            } else {
              tokens[k].type = "task_item_open";
              tokens[k].attrSet("checked", "false");
            }
          }

          const itemCloseIdx = findMatchingClose(
            tokens,
            k,
            "list_item_open",
            "list_item_close",
          );
          if (
            itemCloseIdx !== -1 &&
            tokens[itemCloseIdx].type === "list_item_close"
          ) {
            tokens[itemCloseIdx].type = "task_item_close";
          }
        }
      }
    }
  });
}

function tableCellParagraphsPlugin(md: MarkdownIt) {
  md.core.ruler.after("inline", "table_cell_paragraphs", (state) => {
    const tokens = state.tokens;
    const out: Token[] = [];

    for (let i = 0; i < tokens.length; i++) {
      const token = tokens[i];
      const prev = tokens[i - 1];
      const next = tokens[i + 1];

      if (
        token.type === "inline" &&
        (prev?.type === "th_open" || prev?.type === "td_open") &&
        (next?.type === "th_close" || next?.type === "td_close")
      ) {
        const open = new state.Token("paragraph_open", "p", 1);
        open.level = token.level;
        const inline = new state.Token("inline", "", 0);
        Object.assign(inline, token);
        inline.level = token.level + 1;
        const close = new state.Token("paragraph_close", "p", -1);
        close.level = token.level;

        out.push(open, inline, close);
      } else {
        out.push(token);
      }
    }

    state.tokens = out;
  });
}

function findInlineToken(tokens: Token[], fromIdx: number): number {
  for (let i = fromIdx + 1; i < tokens.length; i++) {
    if (tokens[i].type === "inline") return i;
    if (
      tokens[i].type === "list_item_close" ||
      tokens[i].type === "task_item_close" ||
      tokens[i].type === "bullet_list_close"
    ) {
      break;
    }
  }
  return -1;
}

function findMatchingClose(
  tokens: Token[],
  openIdx: number,
  openType: string,
  closeType: string,
): number {
  let depth = 1;
  for (let i = openIdx + 1; i < tokens.length; i++) {
    if (tokens[i].type === openType) depth++;
    if (tokens[i].type === closeType) {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function isTaskItemContent(content: string): boolean {
  return /^\[([ xX])\]\s/.test(content);
}

function stripTaskPrefix(children: Token[], prefixLen: number) {
  let remaining = prefixLen;
  for (let i = 0; i < children.length && remaining > 0; i++) {
    const child = children[i];
    if (child.type === "text" && child.content) {
      if (child.content.length <= remaining) {
        remaining -= child.content.length;
        child.content = "";
      } else {
        child.content = child.content.slice(remaining);
        remaining = 0;
      }
    }
  }
}

function clipPlugin(md: MarkdownIt) {
  md.block.ruler.before(
    "html_block",
    "clip",
    (
      state: StateBlock,
      startLine: number,
      _endLine: number,
      silent: boolean,
    ) => {
      const pos = state.bMarks[startLine] + state.tShift[startLine];
      const max = state.eMarks[startLine];
      const line = state.src.slice(pos, max);

      const clipMatch = line.match(
        /^<Clip\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*(?:\/>|><\/Clip>)/i,
      );
      if (clipMatch) {
        if (!silent) {
          const token = state.push("clip", "", 0);
          token.attrSet("src", clipMatch[1]);
          token.map = [startLine, startLine + 1];
          token.content = clipMatch[0];
        }
        state.line = startLine + 1;
        return true;
      }

      const iframeMatch = line.match(
        /^<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*>\s*<\/iframe>/i,
      );
      if (iframeMatch) {
        if (!silent) {
          const token = state.push("clip", "", 0);
          token.attrSet("src", iframeMatch[1]);
          token.map = [startLine, startLine + 1];
          token.content = iframeMatch[0];
        }
        state.line = startLine + 1;
        return true;
      }

      return false;
    },
  );
}

function fileAttachmentPlugin(md: MarkdownIt) {
  md.block.ruler.before(
    "paragraph",
    "file_attachment",
    (
      state: StateBlock,
      startLine: number,
      _endLine: number,
      silent: boolean,
    ) => {
      const pos = state.bMarks[startLine] + state.tShift[startLine];
      const max = state.eMarks[startLine];
      const line = state.src.slice(pos, max);

      if (!line.startsWith("[")) return false;

      const closeBracket = line.indexOf("](");
      if (closeBracket === -1) return false;

      const name = line.slice(1, closeBracket);
      if (name.includes("]")) return false;

      const urlStart = closeBracket + 2;
      const assetPrefix = "asset://localhost/";
      const portablePrefix = "attachment://";
      const isAssetUrl = line.startsWith(assetPrefix, urlStart);
      const isPortableUrl = line.startsWith(portablePrefix, urlStart);
      if (!isAssetUrl && !isPortableUrl) return false;

      let depth = 1;
      let i = urlStart;
      while (i < line.length && depth > 0) {
        if (line[i] === "(") depth++;
        else if (line[i] === ")") depth--;
        if (depth > 0) i++;
      }
      if (depth !== 0) return false;

      // Must be the entire line (trailing whitespace OK)
      if (line.slice(i + 1).trim() !== "") return false;

      if (silent) return true;

      const url = line.slice(urlStart, i);
      const attachmentId = isPortableUrl
        ? url.slice(portablePrefix.length)
        : null;
      if (
        attachmentId !== null &&
        !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(
          attachmentId,
        )
      ) {
        return false;
      }
      const token = state.push("file_attachment", "", 0);
      token.attrSet("name", name);
      if (attachmentId) token.attrSet("attachment-id", attachmentId);
      else token.attrSet("src", url);
      token.map = [startLine, startLine + 1];
      token.content = line;
      state.line = startLine + 1;
      return true;
    },
  );
}

function mentionPlugin(md: MarkdownIt) {
  md.inline.ruler.before(
    "html_inline",
    "mention",
    (state: StateInline, silent: boolean) => {
      const start = state.pos;
      if (state.src.charCodeAt(start) !== 0x3c /* < */) return false;

      const match = state.src
        .slice(start)
        .match(
          /^<mention\s+data-id="([^"]*?)"\s+data-type="([^"]*?)"\s+data-label="([^"]*?)"\s*><\/mention>/,
        );
      if (!match) return false;

      if (!silent) {
        const token = state.push("mention", "", 0);
        token.attrSet("data-id", match[1]);
        token.attrSet("data-type", match[2]);
        token.attrSet("data-label", match[3]);
        token.content = match[0];
      }

      state.pos += match[0].length;
      return true;
    },
  );
}

// Markdown collapses consecutive blank lines, so empty paragraphs would be lost
// on roundtrip. We use the line maps that markdown-it attaches to top-level
// block tokens to detect blank lines (leading, trailing, and between blocks)
// and emit explicit empty paragraph tokens.
function emptyParagraphsPlugin(md: MarkdownIt) {
  md.core.ruler.after("block", "empty_paragraphs", (state) => {
    const tokens = state.tokens;
    const out: Token[] = [];
    const totalLines = countLines(state.src);

    const pushEmpty = () => {
      out.push(
        new state.Token("paragraph_open", "p", 1),
        new state.Token("paragraph_close", "p", -1),
      );
    };

    let prevEndLine = -1;
    for (const token of tokens) {
      const isTopLevelBlockOpen =
        token.level === 0 && token.nesting >= 0 && token.map !== null;

      if (isTopLevelBlockOpen) {
        // Leading: the entire gap before the first block is empty paragraphs.
        // Between blocks: one blank line is normal separation; the rest are
        // empty paragraphs.
        const extras =
          prevEndLine === -1 ? token.map![0] : token.map![0] - prevEndLine - 1;
        for (let i = 0; i < extras; i++) pushEmpty();
      }

      out.push(token);

      if (isTopLevelBlockOpen) {
        prevEndLine = token.map![1];
      }
    }

    if (prevEndLine === -1) {
      // No blocks at all — every line plus the implicit "current" line is an
      // empty paragraph. Empty input still produces one empty paragraph, which
      // matches the editor's default "blank document" state.
      for (let i = 0; i <= totalLines; i++) pushEmpty();
    } else if (totalLines > prevEndLine) {
      const trailing = totalLines - prevEndLine;
      for (let i = 0; i < trailing; i++) pushEmpty();
    }

    state.tokens = out;
  });
}

function countLines(src: string): number {
  if (src === "") return 0;
  const newlines = (src.match(/\n/g) || []).length;
  return newlines + (src.endsWith("\n") ? 0 : 1);
}

// ---------------------------------------------------------------------------
// JSON post-processing: lift standalone images out of paragraphs (md2json),
// and wrap block-level images back into paragraphs (json2md).
// ---------------------------------------------------------------------------

let _parser: MarkdownParser | null = null;

export function getParser(): MarkdownParser {
  if (_parser) return _parser;

  const md = MarkdownIt("commonmark", { html: false });

  md.use(strikethroughPlugin);
  md.use(underlinePlugin);
  md.use(highlightPlugin);
  md.use(hardBreakTagPlugin);
  md.enable("table");
  md.use(tableCellParagraphsPlugin);
  md.use(taskListPlugin);
  md.use(clipPlugin);
  md.use(fileAttachmentPlugin);
  md.use(mentionPlugin);
  md.use(emptyParagraphsPlugin);

  _parser = new MarkdownParser(markdownSchema, md, {
    blockquote: { block: "blockquote" },
    paragraph: { block: "paragraph" },
    list_item: { block: "listItem" },
    bullet_list: { block: "bulletList" },
    ordered_list: {
      block: "orderedList",
      getAttrs: (tok) => ({ start: +tok.attrGet("start")! || 1 }),
    },
    heading: {
      block: "heading",
      getAttrs: (tok) => ({ level: +tok.tag.slice(1) }),
    },
    code_block: { block: "codeBlock", noCloseToken: true },
    fence: {
      block: "codeBlock",
      getAttrs: (tok) => ({ language: tok.info || "" }),
      noCloseToken: true,
    },
    hr: { node: "horizontalRule" },
    image: {
      node: "image",
      getAttrs: (tok) => {
        const rawTitle = tok.attrGet("title") || undefined;
        const metadata = parseImageTitleMetadata(rawTitle);
        return {
          src: tok.attrGet("src"),
          alt: (tok.children?.[0] && tok.children[0].content) || "",
          title: metadata.title,
          attachmentId: null,
          editorWidth: metadata.editorWidth ?? DEFAULT_EDITOR_WIDTH,
        };
      },
    },
    hardbreak: { node: "hardBreak" },
    table: { block: "table" },
    thead: { ignore: true },
    tbody: { ignore: true },
    tr: { block: "tableRow" },
    th: { block: "tableHeader" },
    td: { block: "tableCell" },

    em: { mark: "italic" },
    strong: { mark: "bold" },
    s: { mark: "strike" },
    link: {
      mark: "link",
      getAttrs: (tok) => ({
        href: tok.attrGet("href"),
        target: tok.attrGet("target"),
      }),
    },
    code_inline: { mark: "code", noCloseToken: true },

    underline: { mark: "underline" },
    highlight: { mark: "highlight" },
    task_list: { block: "taskList" },
    task_item: {
      block: "taskItem",
      getAttrs: (tok) => ({
        checked: tok.attrGet("checked") === "true",
      }),
    },
    clip: {
      node: "clip",
      getAttrs: (tok) => ({ src: tok.attrGet("src") }),
    },
    file_attachment: {
      node: "fileAttachment",
      getAttrs: (tok) => ({
        attachmentId: tok.attrGet("attachment-id"),
        name: tok.attrGet("name") ?? "",
        mimeType: "",
        src: tok.attrGet("src"),
        path: null,
        size: null,
      }),
    },
    mention: {
      node: "mention-@",
      getAttrs: (tok) => ({
        id: tok.attrGet("data-id"),
        type: tok.attrGet("data-type"),
        label: tok.attrGet("data-label"),
      }),
    },
  });

  return _parser;
}

// ---------------------------------------------------------------------------
// Serializer
