type DocNode = {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: DocNode[];
  marks?: unknown[];
  text?: string;
};

const LIST_TYPES = new Set(["bulletList", "orderedList", "taskList"]);

export function plainTextToDoc(title: string, bodyText: string): string {
  const content: DocNode[] = [];
  if (title !== "") {
    content.push({
      type: "heading",
      attrs: { level: 1 },
      content: [{ type: "text", text: title }],
    });
  }
  for (const line of bodyText.split("\n")) {
    content.push(
      line === ""
        ? { type: "paragraph" }
        : { type: "paragraph", content: [{ type: "text", text: line }] },
    );
  }
  return JSON.stringify({ type: "doc", content });
}

function collectText(node: DocNode): string {
  if (typeof node.text === "string") return node.text;
  if (!Array.isArray(node.content)) return "";
  return node.content.map(collectText).join("");
}

function blockToLines(node: DocNode): string[] {
  if (node.type && LIST_TYPES.has(node.type) && Array.isArray(node.content)) {
    return node.content.map((item) => `- ${collectText(item)}`);
  }
  return [collectText(node)];
}

export function docToPlainText(body: string): { title: string; text: string } {
  let doc: DocNode | null = null;
  if (body.trim() !== "") {
    try {
      const parsed: unknown = JSON.parse(body);
      if (parsed && typeof parsed === "object") doc = parsed as DocNode;
    } catch {
      doc = null;
    }
  }
  if (!doc || doc.type !== "doc" || !Array.isArray(doc.content)) {
    return { title: "", text: "" };
  }

  const blocks = [...doc.content];
  let title = "";
  const first = blocks[0];
  if (first && first.type === "heading" && first.attrs?.level === 1) {
    title = collectText(first);
    blocks.shift();
  }
  return { title, text: blocks.flatMap(blockToLines).join("\n") };
}

// Desktop's markdown notes keep "# Title" as the first line; strip it on read
// so edits don't duplicate the title (mirror of docToPlainText's heading strip).
export function stripMarkdownTitle(body: string): {
  title: string;
  text: string;
} {
  const lines = body.split("\n");
  if (lines[0]?.startsWith("# ")) {
    const title = lines[0].slice(2).trim();
    let rest = lines.slice(1);
    if (rest[0]?.trim() === "") rest = rest.slice(1);
    return { title, text: rest.join("\n") };
  }
  return { title: "", text: body };
}

export function markdownWithTitle(title: string, text: string): string {
  return title === "" ? text : `# ${title}\n\n${text}`;
}

function parseDoc(body: string): DocNode | null {
  if (body.trim() === "") return null;
  try {
    const parsed: unknown = JSON.parse(body);
    if (
      parsed &&
      typeof parsed === "object" &&
      (parsed as DocNode).type === "doc" &&
      Array.isArray((parsed as DocNode).content)
    ) {
      return parsed as DocNode;
    }
  } catch {}
  return null;
}

function isPlainTextBlock(node: DocNode): boolean {
  if (node.type !== "paragraph") return false;
  if (!node.content) return true;
  return node.content.every(
    (child) =>
      child.type === "text" &&
      typeof child.text === "string" &&
      (child.marks?.length ?? 0) === 0,
  );
}

// A doc the plain-text mobile editor can round-trip without losing structure:
// an optional leading level-1 heading followed by unmarked text paragraphs.
export function isPlainTextDoc(body: string): boolean {
  if (body.trim() === "") return true;
  const doc = parseDoc(body);
  if (!doc?.content) return false;
  return doc.content.every((block, index) =>
    index === 0 && block.type === "heading" && block.attrs?.level === 1
      ? (block.content ?? []).every(
          (child) => child.type === "text" && (child.marks?.length ?? 0) === 0,
        )
      : isPlainTextBlock(block),
  );
}

// Title-only edits must not flatten the body; patch just the heading (or the
// markdown title line) and leave everything else untouched. Null = skip write.
export function retitleBody(
  body: string,
  bodyFormat: string,
  title: string,
): string | null {
  if (bodyFormat === "markdown") {
    const { text } = stripMarkdownTitle(body);
    return markdownWithTitle(title, text);
  }
  const doc = parseDoc(body);
  if (!doc?.content) return null;
  const blocks = [...doc.content];
  const heading: DocNode = {
    type: "heading",
    attrs: { level: 1 },
    ...(title === "" ? {} : { content: [{ type: "text", text: title }] }),
  };
  if (blocks[0]?.type === "heading" && blocks[0].attrs?.level === 1) {
    if (title === "") blocks.shift();
    else blocks[0] = { ...blocks[0], content: heading.content };
  } else if (title !== "") {
    blocks.unshift(heading);
  } else {
    return null;
  }
  if (blocks.length === 0) blocks.push({ type: "paragraph" });
  return JSON.stringify({ ...doc, content: blocks });
}
