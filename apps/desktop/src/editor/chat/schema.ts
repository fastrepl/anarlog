import { type NodeSpec, Schema } from "prosemirror-model";

import { attachmentNodeSpec, mentionNodeSpec } from "../node-views";

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

  "mention-@": mentionNodeSpec,
  attachment: attachmentNodeSpec,
};

export const chatSchema = new Schema({ nodes });
