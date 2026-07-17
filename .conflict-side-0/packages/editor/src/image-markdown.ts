const EDITOR_WIDTH_PREFIX = "char-editor-width=";
const MIN_EDITOR_WIDTH = 15;
const MAX_EDITOR_WIDTH = 100;

export const DEFAULT_EDITOR_WIDTH = 80;

function clampEditorWidth(value: number) {
  return Math.min(MAX_EDITOR_WIDTH, Math.max(MIN_EDITOR_WIDTH, value));
}

function normalizeEditorWidth(value: unknown) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return null;
  }

  return clampEditorWidth(Math.round(value));
}

export function parseImageTitleMetadata(title?: string | null) {
  if (!title) {
    return { editorWidth: null, title: null };
  }

  const match = title.match(/^char-editor-width=(\d{1,3})(?:\|(.*))?$/s);
  if (!match) {
    return { editorWidth: null, title };
  }

  const editorWidth = normalizeEditorWidth(Number(match[1]));
  const parsedTitle = match[2] || null;
  return { editorWidth, title: parsedTitle };
}

function serializeImageTitleMetadata({
  editorWidth,
  title,
}: {
  editorWidth?: unknown;
  title?: string | null;
}) {
  const normalizedTitle = title || null;
  const normalizedWidth = normalizeEditorWidth(editorWidth);

  if (!normalizedWidth) {
    return normalizedTitle;
  }

  return normalizedTitle
    ? `${EDITOR_WIDTH_PREFIX}${normalizedWidth}|${normalizedTitle}`
    : `${EDITOR_WIDTH_PREFIX}${normalizedWidth}`;
}

export function serializeMarkdownImage({
  src,
  alt,
  title,
  editorWidth,
  escapeAlt = (value) => value.replace(/]/g, "\\]"),
}: {
  src?: string | null;
  alt?: string | null;
  title?: string | null;
  editorWidth?: unknown;
  escapeAlt?: (value: string) => string;
}) {
  const escapedAlt = escapeAlt(alt || "");
  const escapedSrc = src ? src.replace(/[()]/g, "\\$&") : "";
  const metadataTitle = serializeImageTitleMetadata({ editorWidth, title });
  const titlePart = metadataTitle
    ? ` "${metadataTitle.replace(/"/g, '\\"')}"`
    : "";

  return `![${escapedAlt}](${escapedSrc}${titlePart})`;
}
