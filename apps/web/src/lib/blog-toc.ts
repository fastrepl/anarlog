export type BlogHeading = {
  id: string;
  text: string;
};

type HastNode = {
  type?: unknown;
  tagName?: unknown;
  name?: unknown;
  value?: unknown;
  properties?: {
    id?: unknown;
  };
  attributes?: unknown;
  children?: unknown;
};

function isObject(value: unknown): value is HastNode {
  return typeof value === "object" && value !== null;
}

function extractText(node: unknown): string {
  if (!isObject(node)) {
    return "";
  }

  if (node.type === "text" && typeof node.value === "string") {
    return node.value;
  }

  if (!Array.isArray(node.children)) {
    return "";
  }

  return node.children.map(extractText).join("");
}

function getH2Id(node: HastNode): string | null {
  if (node.type === "element" && node.tagName === "h2") {
    return typeof node.properties?.id === "string"
      ? node.properties.id.trim()
      : "";
  }

  if (
    (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") &&
    node.name === "h2"
  ) {
    if (!Array.isArray(node.attributes)) {
      return "";
    }

    for (const attr of node.attributes) {
      if (
        isObject(attr) &&
        attr.type === "mdxJsxAttribute" &&
        attr.name === "id" &&
        typeof attr.value === "string"
      ) {
        return attr.value.trim();
      }
    }

    return "";
  }

  return null;
}

function collectH2Headings(node: unknown, headings: BlogHeading[]): void {
  if (!isObject(node)) {
    return;
  }

  const id = getH2Id(node);
  if (id !== null) {
    if (id.length > 0) {
      const text = extractText(node).trim();
      if (text.length > 0) {
        headings.push({ id, text });
      }
    }
    return;
  }

  if (!Array.isArray(node.children)) {
    return;
  }

  for (const child of node.children) {
    collectH2Headings(child, headings);
  }
}

/** Extracts top-level `h2` headings with non-empty `id` attributes from a HAST tree. */
export function extractBlogHeadingsFromHast(tree: unknown): BlogHeading[] {
  const headings: BlogHeading[] = [];
  collectH2Headings(tree, headings);
  return headings;
}

/** Rehype plugin that appends extracted `h2` headings into `headings` after slug generation. */
export function rehypeExtractH2Headings(headings: BlogHeading[]) {
  return (tree: unknown): void => {
    headings.push(...extractBlogHeadingsFromHast(tree));
  };
}

/** Returns whether a blog article has enough sections to show a table of contents. */
export function shouldRenderBlogToc(
  headings: ReadonlyArray<BlogHeading>,
): boolean {
  return headings.length >= 3;
}
