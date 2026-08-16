export type EditorFormat =
  | "heading"
  | "bold"
  | "italic"
  | "bullet"
  | "checklist";

type TextSelection = { start: number; end: number };

const BLOCK_FORMATS = {
  heading: { prefix: "# ", pattern: /^#{1,6} / },
  bullet: { prefix: "- ", pattern: /^(?:[-*+] )(?!\[(?: |x|X)\] )/ },
  checklist: { prefix: "- [ ] ", pattern: /^- \[(?: |x|X)\] / },
} as const;

const ANY_BLOCK_PREFIX = /^(?:#{1,6} |- \[(?: |x|X)\] |[-*+] )/;

function normalizeSelection(text: string, selection: TextSelection) {
  const start = Math.max(0, Math.min(text.length, selection.start));
  const end = Math.max(0, Math.min(text.length, selection.end));
  return start <= end ? { start, end } : { start: end, end: start };
}

function wrapSelection(
  text: string,
  selection: TextSelection,
  marker: string,
): { text: string; selection: TextSelection } {
  const { start, end } = normalizeSelection(text, selection);
  const selected = text.slice(start, end);
  const markerLength = marker.length;
  const isSurrounded =
    start >= markerLength &&
    text.slice(start - markerLength, start) === marker &&
    text.slice(end, end + markerLength) === marker;

  if (isSurrounded) {
    return {
      text:
        text.slice(0, start - markerLength) +
        selected +
        text.slice(end + markerLength),
      selection: {
        start: start - markerLength,
        end: end - markerLength,
      },
    };
  }

  if (
    selected.length >= markerLength * 2 &&
    selected.startsWith(marker) &&
    selected.endsWith(marker)
  ) {
    const unwrapped = selected.slice(markerLength, -markerLength);
    return {
      text: text.slice(0, start) + unwrapped + text.slice(end),
      selection: { start, end: start + unwrapped.length },
    };
  }

  return {
    text: text.slice(0, start) + marker + selected + marker + text.slice(end),
    selection: {
      start: start + markerLength,
      end: end + markerLength,
    },
  };
}

function formatLines(
  text: string,
  selection: TextSelection,
  format: keyof typeof BLOCK_FORMATS,
): { text: string; selection: TextSelection } {
  const { start, end } = normalizeSelection(text, selection);
  const lineStart = start === 0 ? 0 : text.lastIndexOf("\n", start - 1) + 1;
  const effectiveEnd = end > start && text[end - 1] === "\n" ? end - 1 : end;
  const nextLineBreak = text.indexOf("\n", effectiveEnd);
  const lineEnd = nextLineBreak === -1 ? text.length : nextLineBreak;
  const lines = text.slice(lineStart, lineEnd).split("\n");
  const target = BLOCK_FORMATS[format];
  const removeTarget = lines.every((line) => target.pattern.test(line));
  const transformedLines = lines.map((line) => {
    if (removeTarget) return line.replace(target.pattern, "");
    return target.prefix + line.replace(ANY_BLOCK_PREFIX, "");
  });
  const transformed = transformedLines.join("\n");
  const nextText = text.slice(0, lineStart) + transformed + text.slice(lineEnd);

  if (start !== end) {
    return {
      text: nextText,
      selection: { start: lineStart, end: lineStart + transformed.length },
    };
  }

  const originalLine = lines[0] ?? "";
  const oldPrefixLength = removeTarget
    ? (originalLine.match(target.pattern)?.[0].length ?? 0)
    : (originalLine.match(ANY_BLOCK_PREFIX)?.[0].length ?? 0);
  const newPrefixLength = removeTarget ? 0 : target.prefix.length;
  const contentOffset = Math.max(0, start - lineStart - oldPrefixLength);
  const cursor = Math.min(
    lineStart + transformed.length,
    lineStart + newPrefixLength + contentOffset,
  );
  return { text: nextText, selection: { start: cursor, end: cursor } };
}

export function applyEditorFormat(
  text: string,
  selection: TextSelection,
  format: EditorFormat,
): {
  text: string;
  selection: TextSelection;
  bodyFormat: "markdown";
} {
  const formatted =
    format === "bold"
      ? wrapSelection(text, selection, "**")
      : format === "italic"
        ? wrapSelection(text, selection, "_")
        : formatLines(text, selection, format);
  return { ...formatted, bodyFormat: "markdown" };
}
