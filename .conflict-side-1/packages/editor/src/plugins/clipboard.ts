import type { Fragment, Node as PMNode, Slice } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";

import { serializeMarkdownImage } from "../image-markdown";

const BLOCK_SEPARATOR = "\n\n";

function imageLeafText(node: PMNode) {
  return serializeMarkdownImage({
    src: node.attrs.src,
    alt: node.attrs.alt,
    title: node.attrs.title,
    editorWidth: node.attrs.editorWidth,
  });
}

function leafText(node: PMNode) {
  if (node.type.name === "image") {
    return imageLeafText(node);
  }

  return node.textContent;
}

function shouldSeparateBlock(node: PMNode, nodeText: string) {
  return (
    node.isBlock && (node.isTextblock || (node.isLeaf && nodeText.length > 0))
  );
}

function fragmentTextBetweenBlocks(fragment: Fragment) {
  let text = "";
  let firstBlock = true;

  fragment.nodesBetween(0, fragment.size, (node) => {
    const nodeText = node.isText
      ? node.text || ""
      : node.isLeaf
        ? leafText(node)
        : "";

    if (shouldSeparateBlock(node, nodeText)) {
      if (firstBlock) {
        firstBlock = false;
      } else {
        text += BLOCK_SEPARATOR;
      }
    }

    text += nodeText;
  });

  return text;
}

export function serializeClipboardText(slice: Slice) {
  return fragmentTextBetweenBlocks(slice.content);
}

export function clipboardPlugin() {
  return new Plugin({
    key: new PluginKey("clipboard"),
    props: {
      clipboardTextSerializer(slice) {
        return serializeClipboardText(slice);
      },
    },
  });
}
