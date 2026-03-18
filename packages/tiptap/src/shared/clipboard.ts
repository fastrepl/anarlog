import {
  Extension,
  getTextBetween,
  getTextSerializersFromSchema,
  type Editor,
} from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

type ResolveClipboardImageSrc = (src: string) => Promise<string | null>;
type WriteClipboardHtml = (html: string, altText: string) => Promise<void>;

const IMAGE_TAG_REGEX = /<img\b[^>]*>/gi;
const IMAGE_SRC_REGEX = /\bsrc\s*=\s*(['"])(.*?)\1/i;
const IMAGE_WIDTH_REGEX = /\bdata-editor-width\s*=\s*(['"])(.*?)\1/i;
const STYLE_REGEX = /\bstyle\s*=\s*(['"])(.*?)\1/i;
const MARKDOWN_IMAGE_REGEX = /!\[([^\]]*)\]\((\S+?)(?:\s+"([^"]*)")?\)/g;

function isWholeDocumentSelection(editor: Editor) {
  const { doc, selection } = editor.state;

  return selection.from <= 1 && selection.to >= doc.content.size - 1;
}

function getSelectionText(editor: Editor) {
  const { state, schema } = editor;
  const { doc, selection } = state;
  const { ranges } = selection;
  const from = Math.min(...ranges.map((range) => range.$from.pos));
  const to = Math.max(...ranges.map((range) => range.$to.pos));

  if (isWholeDocumentSelection(editor)) {
    return editor.getMarkdown();
  }

  const textSerializers = getTextSerializersFromSchema(schema);
  const range = { from, to };

  return getTextBetween(doc, range, {
    textSerializers,
  });
}

function selectionContainsImages(editor: Editor) {
  const { selection } = editor.state;
  let hasImage = false;

  editor.state.doc.nodesBetween(selection.from, selection.to, (node) => {
    if (node.type.name === "image") {
      hasImage = true;
      return false;
    }

    return !hasImage;
  });

  return hasImage;
}

function mergeWidthStyle(style: string, width: string) {
  const withoutWidth = style.replace(/(^|;)\s*width\s*:[^;]+;?/gi, "$1").trim();
  const normalized =
    withoutWidth && !withoutWidth.endsWith(";")
      ? `${withoutWidth};`
      : withoutWidth;

  return normalized ? `${normalized} width: ${width};` : `width: ${width};`;
}

function applyImageWidthStyle(tag: string) {
  const widthMatch = tag.match(IMAGE_WIDTH_REGEX);
  const widthValue = Number(widthMatch?.[2]);

  if (!Number.isFinite(widthValue) || widthValue <= 0) {
    return tag;
  }

  const styleMatch = tag.match(STYLE_REGEX);
  const nextStyle = mergeWidthStyle(styleMatch?.[2] ?? "", `${widthValue}%`);

  if (!styleMatch) {
    return tag.replace("<img", `<img style="${nextStyle}"`);
  }

  return tag.replace(
    styleMatch[0],
    `style=${styleMatch[1]}${nextStyle}${styleMatch[1]}`,
  );
}

export async function rewriteHtmlImageSources(
  html: string,
  resolveImageSrc: ResolveClipboardImageSrc,
) {
  const matches = Array.from(html.matchAll(IMAGE_TAG_REGEX));

  if (!matches.length) {
    return html;
  }

  let result = "";
  let lastIndex = 0;

  for (const match of matches) {
    const index = match.index ?? 0;
    const tag = match[0];
    let nextTag = applyImageWidthStyle(tag);
    const srcMatch = nextTag.match(IMAGE_SRC_REGEX);

    if (srcMatch) {
      const resolvedSrc = await resolveImageSrc(srcMatch[2]);
      if (resolvedSrc) {
        nextTag = nextTag.replace(
          srcMatch[0],
          `src=${srcMatch[1]}${resolvedSrc}${srcMatch[1]}`,
        );
      }
    }

    result += html.slice(lastIndex, index);
    result += nextTag;
    lastIndex = index + tag.length;
  }

  return result + html.slice(lastIndex);
}

export async function rewriteMarkdownImageSources(
  markdown: string,
  resolveImageSrc: ResolveClipboardImageSrc,
) {
  const matches = Array.from(markdown.matchAll(MARKDOWN_IMAGE_REGEX));

  if (!matches.length) {
    return markdown;
  }

  let result = "";
  let lastIndex = 0;

  for (const match of matches) {
    const index = match.index ?? 0;
    const [raw, alt, src, title] = match;
    const resolvedSrc = await resolveImageSrc(src);

    result += markdown.slice(lastIndex, index);
    result += resolvedSrc
      ? title
        ? `![${alt}](${resolvedSrc} "${title}")`
        : `![${alt}](${resolvedSrc})`
      : raw;
    lastIndex = index + raw.length;
  }

  return result + markdown.slice(lastIndex);
}

function handleImageCopy(
  view: EditorView,
  editor: Editor,
  event: ClipboardEvent,
  resolveImageSrc?: ResolveClipboardImageSrc,
  writeClipboardHtml?: WriteClipboardHtml,
) {
  if (!resolveImageSrc || !selectionContainsImages(editor)) {
    return false;
  }

  const { dom, text } = view.serializeForClipboard(
    view.state.selection.content(),
  );
  const html = dom.innerHTML;
  const altTextPromise = isWholeDocumentSelection(editor)
    ? rewriteMarkdownImageSources(getSelectionText(editor), resolveImageSrc)
    : Promise.resolve(text);
  const htmlPromise = rewriteHtmlImageSources(html, resolveImageSrc);

  if (event.clipboardData) {
    event.preventDefault();
    event.clipboardData.clearData();
    event.clipboardData.setData("text/html", html);
    event.clipboardData.setData("text/plain", text);
  }

  if (writeClipboardHtml) {
    void Promise.all([htmlPromise, altTextPromise])
      .then(([nextHtml, altText]) => writeClipboardHtml(nextHtml, altText))
      .catch((error) => {
        console.error("Failed to write rich clipboard data:", error);
      });
    return true;
  }

  if (typeof ClipboardItem !== "undefined" && navigator.clipboard?.write) {
    const clipboardItem = new ClipboardItem({
      "text/plain": altTextPromise.then(
        (value) => new Blob([value], { type: "text/plain" }),
      ),
      "text/html": htmlPromise.then(
        (value) => new Blob([value], { type: "text/html" }),
      ),
    });

    void navigator.clipboard.write([clipboardItem]).catch((error) => {
      console.error("Failed to write rich clipboard data:", error);
    });
    return true;
  }

  return !!event.clipboardData;
}

export const ClipboardTextSerializer = Extension.create<{
  resolveImageSrc?: ResolveClipboardImageSrc;
  writeClipboardHtml?: WriteClipboardHtml;
}>({
  name: "hyprnoteClipboardTextSerializer",

  addOptions() {
    return {
      resolveImageSrc: undefined,
      writeClipboardHtml: undefined,
    };
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("hyprnoteClipboardTextSerializer"),
        props: {
          clipboardTextSerializer: () => getSelectionText(this.editor),
          handleDOMEvents: {
            copy: (view, event) => {
              if (!(event instanceof ClipboardEvent)) {
                return false;
              }

              return handleImageCopy(
                view,
                this.editor,
                event,
                this.options.resolveImageSrc,
                this.options.writeClipboardHtml,
              );
            },
          },
        },
      }),
    ];
  },
});
