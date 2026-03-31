import { type NodeSpec, Schema } from "prosemirror-model";

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

  hardBreak: {
    inline: true,
    group: "inline",
    selectable: false,
    parseDOM: [{ tag: "br" }],
    toDOM() {
      return ["br"];
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
        tag: 'span.mention[data-mention="true"]',
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
        "span",
        {
          class: "mention",
          "data-mention": "true",
          "data-id": node.attrs.id,
          "data-type": node.attrs.type,
          "data-label": node.attrs.label,
        },
        node.attrs.label || "",
      ];
    },
  },

  attachment: {
    group: "inline",
    inline: true,
    atom: true,
    selectable: true,
    attrs: {
      id: { default: null },
      name: { default: "" },
      mimeType: { default: "" },
      url: { default: null },
      size: { default: null },
    },
    parseDOM: [
      {
        tag: 'span[data-type="attachment"]',
        getAttrs(dom) {
          const el = dom as HTMLElement;
          return {
            id: el.getAttribute("data-id"),
            name: el.getAttribute("data-name"),
            mimeType: el.getAttribute("data-mime-type"),
            url: el.getAttribute("data-url"),
            size: el.getAttribute("data-size")
              ? Number(el.getAttribute("data-size"))
              : null,
          };
        },
      },
    ],
    toDOM(node) {
      const attrs: Record<string, string> = { "data-type": "attachment" };
      if (node.attrs.id) attrs["data-id"] = node.attrs.id;
      if (node.attrs.name) attrs["data-name"] = node.attrs.name;
      if (node.attrs.mimeType) attrs["data-mime-type"] = node.attrs.mimeType;
      if (node.attrs.url) attrs["data-url"] = node.attrs.url;
      if (node.attrs.size != null) attrs["data-size"] = String(node.attrs.size);
      return ["span", attrs, node.attrs.name || "attachment"];
    },
  },
};

export const chatSchema = new Schema({ nodes });
