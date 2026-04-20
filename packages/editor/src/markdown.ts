import MarkdownIt from "markdown-it";
import type StateBlock from "markdown-it/lib/rules_block/state_block.mjs";
import type StateInline from "markdown-it/lib/rules_inline/state_inline.mjs";
import type Token from "markdown-it/lib/token.mjs";
import {
  MarkdownParser,
  MarkdownSerializer,
  type MarkdownSerializerState,
} from "prosemirror-markdown";
import {
  type MarkSpec,
  type NodeSpec,
  Node as PMNode,
  Schema,
} from "prosemirror-model";

// ---------------------------------------------------------------------------
// Image title metadata helpers (self-contained, no tiptap dependency)
// ---------------------------------------------------------------------------

const EDITOR_WIDTH_PREFIX = "char-editor-width=";
const MIN_EDITOR_WIDTH = 15;
const MAX_EDITOR_WIDTH = 100;
const DEFAULT_EDITOR_WIDTH = 80;

function clampEditorWidth(value: number) {
  return Math.min(MAX_EDITOR_WIDTH, Math.max(MIN_EDITOR_WIDTH, value));
}

function normalizeEditorWidth(value: unknown) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return null;
  }
  return clampEditorWidth(Math.round(value));
}

function parseImageTitleMetadata(title?: string | null) {
  if (!title) {
    return { editorWidth: null, title: null };
  }
  const match = title.match(/^char-editor-width=(\d{1,3})(?:\|(.*))?$/s);
  if (!match) {
    return { editorWidth: null, title };
  }
  const editorWidth = normalizeEditorWidth(Number(match[1]));
  const parsedTitle = match[2] || null;
  return { editorWidth, title: parsedTitle };
}

function serializeImageTitleMetadata({
  editorWidth,
  title,
}: {
  editorWidth?: number | null;
  title?: string | null;
}) {
  const normalizedTitle = title || null;
  const normalizedWidth = normalizeEditorWidth(editorWidth);
  if (!normalizedWidth) {
    return normalizedTitle;
  }
  return normalizedTitle
    ? `${EDITOR_WIDTH_PREFIX}${normalizedWidth}|${normalizedTitle}`
    : `${EDITOR_WIDTH_PREFIX}${normalizedWidth}`;
}

// ---------------------------------------------------------------------------
// Schema – mirrors the editor schema by node/mark names and attrs.
// Image is inline here; standalone images are lifted out of paragraphs via
// JSON post-processing (see liftBlockImages / wrapBlockImages).
// ---------------------------------------------------------------------------

const nodes: Record<string, NodeSpec> = {
  doc: { content: "block+" },

  paragraph: {
    content: "inline*",
    group: "block",
    parseDOM: [{ tag: "p" }],
    toDOM() {
      return ["p", 0];
    },
  },

  text: { group: "inline" },

  heading: {
    content: "inline*",
    group: "block",
    attrs: { level: { default: 1 } },
    defining: true,
    parseDOM: [1, 2, 3, 4, 5, 6].map((level) => ({
      tag: `h${level}`,
      attrs: { level },
    })),
    toDOM(node) {
      return [`h${node.attrs.level}`, 0];
    },
  },

  blockquote: {
    content: "block+",
    group: "block",
    defining: true,
    parseDOM: [{ tag: "blockquote" }],
    toDOM() {
      return ["blockquote", 0];
    },
  },

  codeBlock: {
    content: "text*",
    marks: "",
    group: "block",
    code: true,
    defining: true,
    attrs: { language: { default: "" } },
    parseDOM: [{ tag: "pre", preserveWhitespace: "full" }],
    toDOM() {
      return ["pre", ["code", 0]];
    },
  },

  horizontalRule: {
    group: "block",
    parseDOM: [{ tag: "hr" }],
    toDOM() {
      return ["hr"];
    },
  },

  hardBreak: {
    inline: true,
    group: "inline",
    selectable: false,
    parseDOM: [{ tag: "br" }],
    toDOM() {
      return ["br"];
    },
  },

  bulletList: {
    content: "listItem+",
    group: "block",
    parseDOM: [{ tag: "ul" }],
    toDOM() {
      return ["ul", 0];
    },
  },

  orderedList: {
    content: "listItem+",
    group: "block",
    attrs: { start: { default: 1 } },
    parseDOM: [
      {
        tag: "ol",
        getAttrs(dom) {
          const el = dom as HTMLElement;
          return {
            start: el.hasAttribute("start") ? +el.getAttribute("start")! : 1,
          };
        },
      },
    ],
    toDOM(node) {
      return node.attrs.start === 1
        ? ["ol", 0]
        : ["ol", { start: node.attrs.start }, 0];
    },
  },

  listItem: {
    content: "paragraph block*",
    defining: true,
    parseDOM: [{ tag: "li" }],
    toDOM() {
      return ["li", 0];
    },
  },

  taskList: {
    content: "taskItem+",
    group: "block",
    parseDOM: [{ tag: 'ul[data-type="taskList"]' }],
    toDOM() {
      return ["ul", { "data-type": "taskList" }, 0];
    },
  },

  taskItem: {
    content: "paragraph block*",
    defining: true,
    attrs: { checked: { default: false } },
    parseDOM: [
      {
        tag: 'li[data-type="taskItem"]',
        getAttrs(dom) {
          return {
            checked:
              (dom as HTMLElement).getAttribute("data-checked") === "true",
          };
        },
      },
    ],
    toDOM(node) {
      return [
        "li",
        {
          "data-type": "taskItem",
          "data-checked": node.attrs.checked ? "true" : "false",
        },
        0,
      ];
    },
  },

  image: {
    inline: true,
    group: "inline",
    attrs: {
      src: { default: null },
      alt: { default: null },
      title: { default: null },
      attachmentId: { default: null },
      editorWidth: { default: DEFAULT_EDITOR_WIDTH },
    },
    parseDOM: [
      {
        tag: "img[src]",
        getAttrs(dom) {
          const el = dom as HTMLElement;
          return {
            src: el.getAttribute("src"),
            alt: el.getAttribute("alt"),
            title: el.getAttribute("title"),
          };
        },
      },
    ],
    toDOM(node) {
      return [
        "img",
        { src: node.attrs.src, alt: node.attrs.alt, title: node.attrs.title },
      ];
    },
  },

  clip: {
    group: "block",
    atom: true,
    attrs: { src: { default: null } },
    parseDOM: [
      {
        tag: 'div[data-type="clip"]',
        getAttrs(dom) {
          return { src: (dom as HTMLElement).getAttribute("data-src") };
        },
      },
    ],
    toDOM(node) {
      return ["div", { "data-type": "clip", "data-src": node.attrs.src }];
    },
  },

  fileAttachment: {
    group: "block",
    atom: true,
    attrs: {
      attachmentId: { default: null },
      name: { default: "" },
      mimeType: { default: "" },
      src: { default: null },
      path: { default: null },
      size: { default: null },
    },
    parseDOM: [
      {
        tag: 'div[data-type="file-attachment"]',
        getAttrs(dom) {
          const el = dom as HTMLElement;
          return {
            attachmentId: el.getAttribute("data-attachment-id"),
            name: el.getAttribute("data-name"),
            mimeType: el.getAttribute("data-mime-type"),
            src: el.getAttribute("data-src"),
            size: el.getAttribute("data-size")
              ? Number(el.getAttribute("data-size"))
              : null,
          };
        },
      },
    ],
    toDOM(node) {
      const attrs: Record<string, string> = {
        "data-type": "file-attachment",
      };
      if (node.attrs.attachmentId) {
        attrs["data-attachment-id"] = node.attrs.attachmentId;
      }
      if (node.attrs.name) attrs["data-name"] = node.attrs.name;
      if (node.attrs.mimeType) attrs["data-mime-type"] = node.attrs.mimeType;
      if (node.attrs.src) attrs["data-src"] = node.attrs.src;
      if (node.attrs.size != null) attrs["data-size"] = String(node.attrs.size);
      return ["div", attrs];
    },
  },

  "mention-@": {
    group: "inline",
    inline: true,
    atom: true,
    attrs: {
      id: { default: null },
      type: { default: null },
      label: { default: null },
    },
    parseDOM: [
      {
        tag: "mention",
        getAttrs(dom) {
          const el = dom as HTMLElement;
          return {
            id: el.getAttribute("data-id"),
            type: el.getAttribute("data-type"),
            label: el.getAttribute("data-label"),
          };
        },
      },
    ],
    toDOM(node) {
      return [
        "mention",
        {
          "data-id": node.attrs.id,
          "data-type": node.attrs.type,
          "data-label": node.attrs.label,
        },
      ];
    },
  },
};

const marks: Record<string, MarkSpec> = {
  bold: {
    parseDOM: [{ tag: "strong" }, { tag: "b" }],
    toDOM() {
      return ["strong", 0];
    },
  },

  italic: {
    parseDOM: [{ tag: "em" }, { tag: "i" }],
    toDOM() {
      return ["em", 0];
    },
  },

  underline: {
    parseDOM: [{ tag: "u" }],
    toDOM() {
      return ["u", 0];
    },
  },

  strike: {
    parseDOM: [{ tag: "s" }, { tag: "del" }],
    toDOM() {
      return ["s", 0];
    },
  },

  code: {
    excludes: "_",
    parseDOM: [{ tag: "code" }],
    toDOM() {
      return ["code", 0];
    },
  },

  link: {
    attrs: {
      href: {},
      target: { default: null },
    },
    inclusive: false,
    parseDOM: [
      {
        tag: "a[href]",
        getAttrs(dom) {
          return {
            href: (dom as HTMLElement).getAttribute("href"),
            target: (dom as HTMLElement).getAttribute("target"),
          };
        },
      },
    ],
    toDOM(node) {
      return ["a", { href: node.attrs.href, target: node.attrs.target }, 0];
    },
  },

  highlight: {
    parseDOM: [{ tag: "mark" }],
    toDOM() {
      return ["mark", 0];
    },
  },
};

export const markdownSchema = new Schema({ nodes, marks });

// ---------------------------------------------------------------------------
// markdown-it plugins
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

      // [name](asset://localhost/...) with balanced parens in the URL
      if (!line.startsWith("[")) return false;

      const closeBracket = line.indexOf("](");
      if (closeBracket === -1) return false;

      const name = line.slice(1, closeBracket);
      if (name.includes("]")) return false;

      const urlStart = closeBracket + 2;
      const PREFIX = "asset://localhost/";
      if (!line.startsWith(PREFIX, urlStart)) return false;

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
      const token = state.push("file_attachment", "", 0);
      token.attrSet("name", name);
      token.attrSet("src", url);
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

function liftBlockImages(doc: JSONContent): JSONContent {
  if (!doc.content) return doc;

  const out: JSONContent[] = [];
  for (const node of doc.content) {
    if (
      node.type === "paragraph" &&
      node.content &&
      node.content.length === 1 &&
      node.content[0].type === "image"
    ) {
      out.push(node.content[0]);
    } else {
      out.push(node);
    }
  }
  return { ...doc, content: out };
}

function wrapBlockImages(doc: JSONContent): JSONContent {
  if (!doc.content) return doc;

  const out: JSONContent[] = [];
  for (const node of doc.content) {
    if (node.type === "image") {
      out.push({ type: "paragraph", content: [node] });
    } else {
      out.push(node);
    }
  }
  return { ...doc, content: out };
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

let _parser: MarkdownParser | null = null;

function getParser(): MarkdownParser {
  if (_parser) return _parser;

  const md = MarkdownIt("commonmark", { html: false });

  md.use(strikethroughPlugin);
  md.use(underlinePlugin);
  md.use(highlightPlugin);
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
        attachmentId: null,
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

let _serializer: MarkdownSerializer | null = null;

function getSerializer(): MarkdownSerializer {
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
        const alt = state.esc(node.attrs.alt || "");
        const src = node.attrs.src
          ? node.attrs.src.replace(/[()]/g, "\\$&")
          : "";
        const title = serializeImageTitleMetadata({
          editorWidth: node.attrs.editorWidth,
          title: node.attrs.title,
        });
        const titlePart = title ? ` "${title.replace(/"/g, '\\"')}"` : "";
        state.write(`![${alt}](${src}${titlePart})`);
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
        const src = (node.attrs.src || "")
          .replace(/\(/g, "%28")
          .replace(/\)/g, "%29");
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
// ---------------------------------------------------------------------------

export interface JSONContent {
  type?: string;
  attrs?: Record<string, any>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, any> }[];
  text?: string;
}

export const EMPTY_DOC: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

export function isValidContent(content: unknown): content is JSONContent {
  if (!content || typeof content !== "object") {
    return false;
  }
  const obj = content as Record<string, unknown>;
  return obj.type === "doc" && Array.isArray(obj.content);
}

export function parseJsonContent(raw: string | undefined | null): JSONContent {
  if (typeof raw !== "string" || !raw.trim()) {
    return EMPTY_DOC;
  }
  try {
    const parsed = JSON.parse(raw);
    return isValidContent(parsed) ? parsed : EMPTY_DOC;
  } catch {
    return EMPTY_DOC;
  }
}

export function md2json(markdown: string): JSONContent {
  try {
    const doc = getParser().parse(markdown);
    const json = doc.toJSON() as JSONContent;
    if (!json.content || json.content.length === 0) {
      return { type: "doc", content: [{ type: "paragraph" }] };
    }
    return liftBlockImages(json);
  } catch (error) {
    console.error(error);
    return {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: markdown }],
        },
      ],
    };
  }
}

export function json2md(jsonContent: JSONContent): string {
  try {
    const wrapped = wrapBlockImages(jsonContent);
    const doc = PMNode.fromJSON(markdownSchema, wrapped);
    return getSerializer().serialize(doc);
  } catch (error) {
    console.error(error);
    return "";
  }
}
