import type { JSONContent, PlaceholderFunction } from "@anlg/editor/note";

export const documentTitlePlaceholder: PlaceholderFunction = ({ node, pos }) =>
  pos === 0 && node.type.name === "heading" && node.attrs.level === 1
    ? "Untitled"
    : "";

export function isPlausibleTitle(title: string): boolean {
  return (title.match(/[\p{L}\p{N}]/gu)?.length ?? 0) >= 2;
}

export function isTitleMissing(title: string | null | undefined): boolean {
  const trimmed = title?.trim() ?? "";
  return !trimmed || !isPlausibleTitle(trimmed);
}

export function extractFirstLineTitle(content: JSONContent) {
  const firstBlock = content.content?.[0];
  const title = collectText(firstBlock).trim();

  if (title) {
    return title;
  }

  return collectText(content).trim() ? "" : null;
}

export function removeDocumentTitle(
  content: JSONContent,
  title: string | null | undefined,
) {
  const blocks = content.content ?? [];
  const firstBlock = blocks[0];
  const firstBlockText = collectText(firstBlock).trim();
  const sessionTitle = title?.trim() ?? "";
  const isDocumentTitle =
    firstBlock?.type === "heading" &&
    firstBlock.attrs?.level === 1 &&
    (!firstBlockText || firstBlockText === sessionTitle);

  if (!isDocumentTitle && blocks.length > 0) {
    return content;
  }

  const body = isDocumentTitle ? blocks.slice(1) : blocks;
  return {
    ...content,
    content: body.length > 0 ? body : [{ type: "paragraph" }],
  };
}

export function ensureFirstLineTitle(
  content: JSONContent,
  title: string | null | undefined,
  previousTitle?: string | null,
) {
  const trimmedTitle = title?.trim();
  if (!trimmedTitle || !isPlausibleTitle(trimmedTitle)) {
    return content;
  }

  const blocks = content.content ?? [];
  const firstBlock = blocks[0];
  const titleBlock = buildTitleBlock(trimmedTitle);
  const firstBlockText = collectText(firstBlock).trim();
  const trimmedPreviousTitle = previousTitle?.trim();

  // A first-position level-1 heading only gets replaced (instead of having
  // the new title prepended above it) when it's empty or matches the title
  // this document was last given. Otherwise it's the summary's own content
  // heading, not a stale title slot, and must be preserved underneath.
  if (firstBlock?.type === "heading" && firstBlock.attrs?.level === 1) {
    if (firstBlockText === trimmedTitle) {
      return content;
    }
    const isPriorTitleSlot =
      !firstBlockText ||
      (trimmedPreviousTitle != null && firstBlockText === trimmedPreviousTitle);
    return {
      ...content,
      content: isPriorTitleSlot
        ? [titleBlock, ...blocks.slice(1)]
        : [titleBlock, ...blocks],
    };
  }

  if (firstBlock?.type === "paragraph" && firstBlockText === trimmedTitle) {
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
