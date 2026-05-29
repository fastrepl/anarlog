import { Plugin, PluginKey } from "prosemirror-state";

type JSONContent = {
  type?: string;
  content?: JSONContent[];
};

export function ensureImageTrailingParagraphs<T extends JSONContent>(
  content: T,
): T {
  if (content.type !== "doc" || !content.content) return content;

  const next: JSONContent[] = [];
  for (let i = 0; i < content.content.length; i++) {
    const child = content.content[i];
    next.push(child);
    if (child.type === "image") {
      const after = content.content[i + 1];
      if (!after || after.type !== "paragraph") {
        next.push({ type: "paragraph" });
      }
    }
  }
  if (next.length === content.content.length) return content;
  return { ...content, content: next as T["content"] };
}

export function imageTrailingParagraphPlugin() {
  return new Plugin({
    key: new PluginKey("imageTrailingParagraph"),
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((tr) => tr.docChanged)) return null;

      const { doc, schema } = newState;
      const imageType = schema.nodes.image;
      const paragraphType = schema.nodes.paragraph;
      if (!imageType || !paragraphType) return null;

      const insertions: number[] = [];
      doc.content.forEach((child, offset, index) => {
        if (child.type !== imageType) return;
        const next = doc.maybeChild(index + 1);
        if (!next || next.type !== paragraphType) {
          insertions.push(offset + child.nodeSize);
        }
      });

      if (insertions.length === 0) return null;

      const tr = newState.tr;
      for (let i = insertions.length - 1; i >= 0; i--) {
        tr.insert(insertions[i], paragraphType.create());
      }
      return tr;
    },
  });
}
