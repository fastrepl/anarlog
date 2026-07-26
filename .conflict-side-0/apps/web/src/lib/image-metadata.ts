const EDITOR_WIDTH_PREFIX = "char-editor-width=";

export function stripEditorWidthFromTitle(title?: string | null) {
  if (!title) {
    return undefined;
  }

  const match = title.match(
    new RegExp(`^${EDITOR_WIDTH_PREFIX}\\d{1,3}(?:\\|(.*))?$`, "s"),
  );
  if (!match) {
    return title;
  }

  return match[1] || undefined;
}
