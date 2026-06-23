import { type Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";

import { schema } from "./schema";

const titleHeadingPluginKey = new PluginKey("titleHeading");

export function normalizeTitleHeadingDoc(doc: PMNode) {
  const first = doc.firstChild;
  const heading = schema.nodes.heading;

  if (!first) {
    return schema.node("doc", null, [heading.create({ level: 1 })]);
  }

  if (first.type === heading && first.attrs.level === 1) {
    return doc;
  }

  const children: PMNode[] = [];
  doc.forEach((child) => children.push(child));

  if (first.type === schema.nodes.paragraph || first.type === heading) {
    children[0] = heading.create({ level: 1 }, first.content, first.marks);
  } else {
    children.unshift(heading.create({ level: 1 }));
  }

  return schema.node("doc", null, children);
}

export function titleHeadingPlugin() {
  return new Plugin({
    key: titleHeadingPluginKey,
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((transaction) => transaction.docChanged)) {
        return null;
      }

      const first = newState.doc.firstChild;
      const heading = newState.schema.nodes.heading;
      if (!heading) {
        return null;
      }

      if (first?.type === heading && first.attrs.level === 1) {
        return null;
      }

      const tr = newState.tr;
      if (
        first &&
        (first.type === newState.schema.nodes.paragraph ||
          first.type === heading)
      ) {
        tr.setNodeMarkup(0, heading, { level: 1 });
      } else {
        tr.insert(0, heading.create({ level: 1 }));
      }

      return tr;
    },
  });
}
