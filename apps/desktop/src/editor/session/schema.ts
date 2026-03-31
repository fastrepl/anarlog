import { type MarkSpec, type NodeSpec, Schema } from "prosemirror-model";

import { parseYouTubeUrl } from "@hypr/tiptap/shared";
import {
  DEFAULT_EDITOR_WIDTH,
  normalizeEditorWidth,
  parseImageTitleMetadata,
  stripEditorWidthFromTitle,
} from "@hypr/tiptap/shared";

// Node names match Tiptap for JSON content compatibility.
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
    parseDOM: [{ tag: "ul:not([data-type])" }],
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
    parseDOM: [{ tag: "li:not([data-type])" }],
    toDOM() {
      return ["li", 0];
    },
  },

  taskList: {
    content: "taskItem+",
    group: "block",
    parseDOM: [{ tag: 'ul[data-type="taskList"]' }],
    toDOM() {
      return ["ul", { "data-type": "taskList", class: "task-list" }, 0];
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
    group: "block",
    draggable: true,
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
          const title = el.getAttribute("title");
          const metadata = parseImageTitleMetadata(title);
          return {
            src: el.getAttribute("src"),
            alt: el.getAttribute("alt"),
            title: stripEditorWidthFromTitle(title),
            attachmentId: el.getAttribute("data-attachment-id"),
            editorWidth:
              normalizeEditorWidth(
                Number(el.getAttribute("data-editor-width")),
              ) ??
              metadata.editorWidth ??
              DEFAULT_EDITOR_WIDTH,
          };
        },
      },
    ],
    toDOM(node) {
      const attrs: Record<string, string> = {};
      if (node.attrs.src) attrs.src = node.attrs.src;
      if (node.attrs.alt) attrs.alt = node.attrs.alt;
      if (node.attrs.title) attrs.title = node.attrs.title;
      if (node.attrs.attachmentId) {
        attrs["data-attachment-id"] = node.attrs.attachmentId;
      }
      if (node.attrs.editorWidth) {
        attrs["data-editor-width"] = String(node.attrs.editorWidth);
      }
      return ["img", attrs];
    },
  },

  "mention-@": {
    group: "inline",
    inline: true,
    atom: true,
    selectable: true,
    attrs: {
      id: { default: null },
      type: { default: null },
      label: { default: null },
    },
    parseDOM: [
      {
        tag: 'a.mention[data-mention="true"]',
        getAttrs(dom) {
          const el = dom as HTMLElement;
          return {
            id: el.getAttribute("data-id"),
            type: el.getAttribute("data-type"),
            label: el.getAttribute("data-label"),
          };
        },
      },
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
        "a",
        {
          class: "mention",
          "data-mention": "true",
          "data-id": node.attrs.id,
          "data-type": node.attrs.type,
          "data-label": node.attrs.label,
          href: "javascript:void(0)",
        },
        node.attrs.label || "",
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
          const src = (dom as HTMLElement).getAttribute("data-src");
          const parsed = src ? parseYouTubeUrl(src) : null;
          return parsed ? { src: parsed.embedUrl } : false;
        },
      },
      {
        tag: "iframe[src]",
        getAttrs(dom) {
          const src = (dom as HTMLElement).getAttribute("src");
          const parsed = src ? parseYouTubeUrl(src) : null;
          return parsed ? { src: parsed.embedUrl } : false;
        },
      },
    ],
    toDOM(node) {
      return ["div", { "data-type": "clip", "data-src": node.attrs.src }];
    },
  },
};

const marks: Record<string, MarkSpec> = {
  bold: {
    parseDOM: [
      { tag: "strong" },
      {
        tag: "b",
        getAttrs: (node) =>
          (node as HTMLElement).style.fontWeight !== "normal" && null,
      },
      {
        style: "font-weight=400",
        clearMark: (m) => m.type.name === "bold",
      },
      {
        style: "font-weight",
        getAttrs: (value) =>
          /^(bold(er)?|[5-9]\d{2,})$/.test(value as string) && null,
      },
    ],
    toDOM() {
      return ["strong", 0];
    },
  },

  italic: {
    parseDOM: [
      { tag: "em" },
      {
        tag: "i",
        getAttrs: (node) =>
          (node as HTMLElement).style.fontStyle !== "normal" && null,
      },
      { style: "font-style=italic" },
    ],
    toDOM() {
      return ["em", 0];
    },
  },

  strike: {
    parseDOM: [
      { tag: "s" },
      { tag: "del" },
      {
        style: "text-decoration",
        getAttrs: (value) => (value as string).includes("line-through") && null,
      },
    ],
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
      return [
        "a",
        {
          href: node.attrs.href,
          target: node.attrs.target,
          rel: "noopener noreferrer nofollow",
        },
        0,
      ];
    },
  },

  highlight: {
    parseDOM: [{ tag: "mark" }],
    toDOM() {
      return ["mark", 0];
    },
  },
};

export const schema = new Schema({ nodes, marks });
