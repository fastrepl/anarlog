import type { JSONContent } from "@hypr/editor/note";

export function extractFirstLineTitle(content: JSONContent) {
  const firstBlock = content.content?.[0];
  const title = collectText(firstBlock).trim();

  if (title) {
    return title;
  }

  return collectText(content).trim() ? "" : null;
}

export function ensureFirstLineTitle(
  content: JSONContent,
  title: string | null | undefined,
) {
  const trimmedTitle = title?.trim();
  if (!trimmedTitle) {
    return content;
  }

  const blocks = content.content ?? [];
  const firstBlock = blocks[0];
  const titleBlock = buildTitleBlock(trimmedTitle);

  if (
    (firstBlock?.type === "heading" && firstBlock.attrs?.level === 1) ||
    firstBlock?.type === "paragraph"
  ) {
    if (collectText(firstBlock).trim() === trimmedTitle) {
      return firstBlock.type === "heading" && firstBlock.attrs?.level === 1
        ? content
        : { ...content, content: [titleBlock, ...blocks.slice(1)] };
    }
  }

  if (
    firstBlock?.type === "heading" &&
    firstBlock.attrs?.level === 1 &&
    !collectText(firstBlock).trim()
  ) {
    return { ...content, content: [titleBlock, ...blocks.slice(1)] };
  }

  return { ...content, content: [titleBlock, ...blocks] };
}

export function ensureMarkdownFirstLineTitle(
  markdown: string,
  title: string | null | undefined,
) {
  const trimmedTitle = title?.trim();
  if (!trimmedTitle) {
    return markdown;
  }

  const trimmedMarkdown = markdown.trimStart();
  const firstLineEnd = trimmedMarkdown.indexOf("\n");
  const firstLine =
    firstLineEnd === -1
      ? trimmedMarkdown
      : trimmedMarkdown.slice(0, firstLineEnd);

  if (firstLine === `# ${trimmedTitle}`) {
    return markdown;
  }

  return `# ${trimmedTitle}\n\n${markdown.trimStart()}`.trim();
}

function buildTitleBlock(title: string): JSONContent {
  return {
    type: "heading",
    attrs: { level: 1 },
    content: [{ type: "text", text: title }],
  };
}

function collectText(node?: JSONContent): string {
  if (!node) {
    return "";
  }

  const ownText = typeof node.text === "string" ? node.text : "";
  const childText = node.content?.map(collectText).join("") ?? "";
  return ownText + childText;
}
