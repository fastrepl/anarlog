const EDITOR_WIDTH_PREFIX = "char-editor-width=";
const MIN_EDITOR_WIDTH = 15;
const MAX_EDITOR_WIDTH = 100;
export const DEFAULT_EDITOR_WIDTH = 80;

function clampEditorWidth(value: number) {
  return Math.min(MAX_EDITOR_WIDTH, Math.max(MIN_EDITOR_WIDTH, value));
}

export function normalizeEditorWidth(value: unknown) {
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

  return {
    editorWidth,
    title: parsedTitle,
  };
}

export function serializeImageTitleMetadata({
  editorWidth,
  title,
}: {
  editorWidth?: number | null;
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

export function stripEditorWidthFromTitle(title?: string | null) {
  return parseImageTitleMetadata(title).title ?? undefined;
}
