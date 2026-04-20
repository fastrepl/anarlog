import { type MarkSpec, type NodeSpec, Schema } from "prosemirror-model";

import {
  appLinkNodeSpec,
  imageNodeSpec,
  mentionNodeSpec,
  sessionNodeSpec,
  taskItemNodeSpec,
  taskListNodeSpec,
  fileAttachmentNodeSpec,
} from "../node-views";
import { clipNodeSpec } from "../plugins";

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

  taskList: taskListNodeSpec,
  taskItem: taskItemNodeSpec,
  image: imageNodeSpec,
  fileAttachment: fileAttachmentNodeSpec,
  appLink: appLinkNodeSpec,
  "mention-@": mentionNodeSpec,
  session: sessionNodeSpec,
  clip: clipNodeSpec,
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
          const href = (dom as HTMLElement).getAttribute("href");
          if (href && href.startsWith("asset://")) {
            return false;
          }
          return {
            href,
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
