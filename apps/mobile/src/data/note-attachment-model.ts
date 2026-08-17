const UUID_V4_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export function requireNoteAttachmentFilename(value: string): string {
  if (
    value.length === 0 ||
    value.length > 1024 ||
    value.trim() !== value ||
    hasControlCharacter(value) ||
    value.includes("/") ||
    value.includes("\\") ||
    value === "." ||
    value === ".."
  ) {
    throw new Error("The selected file has an unsupported name.");
  }
  return value;
}

function hasControlCharacter(value: string): boolean {
  return [...value].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 31 || code === 127;
  });
}

export function portableNoteAttachmentMarkdown(
  attachmentId: string,
  filename: string,
): string {
  if (!UUID_V4_PATTERN.test(attachmentId)) {
    throw new Error("The attachment identifier is invalid.");
  }
  const label = requireNoteAttachmentFilename(filename)
    .replaceAll("[", "(")
    .replaceAll("]", ")");
  return `[${label}](attachment://${attachmentId})`;
}

export function insertNoteAttachmentMarkdown(
  text: string,
  selection: { start: number; end: number },
  markdown: string,
): { text: string; selection: { start: number; end: number } } {
  const cursor = Math.max(0, Math.min(text.length, selection.end));
  const before = text.slice(0, cursor);
  const after = text.slice(cursor);
  const prefix = before === "" || before.endsWith("\n") ? "" : "\n";
  const suffix = after === "" || after.startsWith("\n") ? "" : "\n";
  const insertion = `${prefix}${markdown}${suffix}`;
  const nextCursor = cursor + insertion.length;
  return {
    text: before + insertion + after,
    selection: { start: nextCursor, end: nextCursor },
  };
}

export function insertCapturedNoteAttachmentMarkdown(input: {
  capturedText: string;
  capturedSelection: { start: number; end: number };
  currentText: string;
  markdown: string;
}): { text: string; selection: { start: number; end: number } } {
  const selection =
    input.currentText === input.capturedText
      ? input.capturedSelection
      : { start: input.currentText.length, end: input.currentText.length };
  return insertNoteAttachmentMarkdown(
    input.currentText,
    selection,
    input.markdown,
  );
}
