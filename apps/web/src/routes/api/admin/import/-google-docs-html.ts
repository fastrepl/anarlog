const BODY_REGEX = /<body[^>]*>([\s\S]*?)<\/body>/i;
const SIMPLE_CLASS_RULE_REGEX = /\.([A-Za-z0-9_-]+)\{([^}]*)\}/g;
const PARAGRAPH_REGEX = /<p\b[^>]*>[\s\S]*?<\/p>/gi;
const PRE_BLOCK_REGEX = /<pre\b[^>]*>[\s\S]*?<\/pre>/gi;
const OPEN_TAG_REGEX = /<(?!\/)([a-z0-9]+)\b([^>]*)>/gi;
const CLASS_ATTR_REGEX = /\bclass=(["'])(.*?)\1/i;
const SPAN_REGEX = /<span\b([^>]*)>([\s\S]*?)<\/span>/gi;

const MONOSPACE_FONT_MARKERS = [
  "courier",
  "consolas",
  "menlo",
  "monaco",
  "roboto mono",
  "source code",
  "sfmono",
  "monospace",
] as const;

function extractGoogleDocsBodyContent(html: string): string {
  const bodyMatch = html.match(BODY_REGEX);
  return bodyMatch ? bodyMatch[1] : html;
}

function getClassNames(attributes: string): string[] {
  const classMatch = attributes.match(CLASS_ATTR_REGEX);
  if (!classMatch) {
    return [];
  }

  return classMatch[2].split(/\s+/).filter(Boolean);
}

function getClassStyles(html: string): Map<string, string> {
  const classStyles = new Map<string, string>();

  for (const [, className, declarations] of html.matchAll(
    SIMPLE_CLASS_RULE_REGEX,
  )) {
    classStyles.set(className, declarations);
  }

  return classStyles;
}

function isMonospaceStyle(style: string): boolean {
  const normalized = style.toLowerCase();
  return MONOSPACE_FONT_MARKERS.some((font) => normalized.includes(font));
}

function isCompactParagraphStyle(style: string): boolean {
  const normalized = style.toLowerCase().replace(/\s+/g, "");
  return (
    normalized.includes("padding-bottom:0pt") ||
    normalized.includes("padding-bottom:0;")
  );
}

function getParagraphInnerHtml(paragraphHtml: string): string {
  const innerMatch = paragraphHtml.match(/^<p\b[^>]*>([\s\S]*?)<\/p>$/i);
  return innerMatch ? innerMatch[1] : "";
}

function isGoogleDocsCodeParagraph(
  paragraphHtml: string,
  monospaceClasses: Set<string>,
  compactParagraphClasses: Set<string>,
): boolean {
  const paragraphMatch = paragraphHtml.match(/^<p\b([^>]*)>/i);
  if (!paragraphMatch) {
    return false;
  }

  const paragraphClasses = getClassNames(paragraphMatch[1]);
  if (
    !paragraphClasses.some((className) =>
      compactParagraphClasses.has(className),
    )
  ) {
    return false;
  }

  const innerHtml = getParagraphInnerHtml(paragraphHtml);
  if (
    /<(?:img|a|table|ul|ol|li|h[1-6]|div|svg|iframe|video|audio)\b/i.test(
      innerHtml,
    )
  ) {
    return false;
  }

  let sawSpan = false;

  for (const [, tagName, attributes] of innerHtml.matchAll(OPEN_TAG_REGEX)) {
    const normalizedTagName = tagName.toLowerCase();
    if (normalizedTagName === "br") {
      continue;
    }

    if (normalizedTagName !== "span") {
      return false;
    }

    sawSpan = true;
    const spanClasses = getClassNames(attributes);
    if (!spanClasses.some((className) => monospaceClasses.has(className))) {
      return false;
    }
  }

  if (!sawSpan) {
    return false;
  }

  const visibleText = innerHtml
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/g, " ")
    .trim();

  return visibleText.length > 0;
}

function convertParagraphsToCodeBlock(paragraphs: string[]): string {
  const lines = paragraphs.map((paragraph) => getParagraphInnerHtml(paragraph));
  return `<pre><code>${lines.join("\n")}</code></pre>`;
}

function wrapMonospaceSpansInCode(
  html: string,
  monospaceClasses: Set<string>,
): string {
  return html.replace(
    SPAN_REGEX,
    (fullMatch, attributes: string, innerHtml: string) => {
      const spanClasses = getClassNames(attributes);
      if (!spanClasses.some((className) => monospaceClasses.has(className))) {
        return fullMatch;
      }

      const visibleText = innerHtml
        .replace(/<[^>]+>/g, "")
        .replace(/&nbsp;/g, " ")
        .trim();

      if (!visibleText) {
        return fullMatch;
      }

      return `<code>${innerHtml}</code>`;
    },
  );
}

function normalizeInlineMonospaceSpans(
  html: string,
  monospaceClasses: Set<string>,
): string {
  let normalized = "";
  let lastIndex = 0;

  for (const preBlockMatch of html.matchAll(PRE_BLOCK_REGEX)) {
    const preBlockHtml = preBlockMatch[0];
    const preBlockIndex = preBlockMatch.index ?? 0;

    normalized += wrapMonospaceSpansInCode(
      html.slice(lastIndex, preBlockIndex),
      monospaceClasses,
    );
    normalized += preBlockHtml;
    lastIndex = preBlockIndex + preBlockHtml.length;
  }

  normalized += wrapMonospaceSpansInCode(
    html.slice(lastIndex),
    monospaceClasses,
  );

  return normalized;
}

export function normalizeGoogleDocsBodyContent(html: string): string {
  const bodyContent = extractGoogleDocsBodyContent(html).replace(
    /&nbsp;/g,
    " ",
  );
  const classStyles = getClassStyles(html);

  const monospaceClasses = new Set(
    [...classStyles.entries()]
      .filter(([, style]) => isMonospaceStyle(style))
      .map(([className]) => className),
  );

  const compactParagraphClasses = new Set(
    [...classStyles.entries()]
      .filter(([, style]) => isCompactParagraphStyle(style))
      .map(([className]) => className),
  );

  if (!monospaceClasses.size) {
    return bodyContent;
  }

  let normalized = "";
  let lastIndex = 0;
  let pendingCodeParagraphs: string[] = [];

  function flushPendingCodeParagraphs() {
    if (!pendingCodeParagraphs.length) {
      return;
    }

    normalized += convertParagraphsToCodeBlock(pendingCodeParagraphs);
    pendingCodeParagraphs = [];
  }

  for (const paragraphMatch of bodyContent.matchAll(PARAGRAPH_REGEX)) {
    const paragraphHtml = paragraphMatch[0];
    const paragraphIndex = paragraphMatch.index ?? 0;
    const betweenParagraphs = bodyContent.slice(lastIndex, paragraphIndex);

    if (pendingCodeParagraphs.length) {
      if (betweenParagraphs.trim() !== "") {
        flushPendingCodeParagraphs();
        normalized += betweenParagraphs;
      }
    } else {
      normalized += betweenParagraphs;
    }

    if (
      isGoogleDocsCodeParagraph(
        paragraphHtml,
        monospaceClasses,
        compactParagraphClasses,
      )
    ) {
      pendingCodeParagraphs.push(paragraphHtml);
    } else {
      flushPendingCodeParagraphs();
      normalized += paragraphHtml;
    }

    lastIndex = paragraphIndex + paragraphHtml.length;
  }

  flushPendingCodeParagraphs();
  normalized += bodyContent.slice(lastIndex);

  return normalizeInlineMonospaceSpans(normalized, monospaceClasses);
}
