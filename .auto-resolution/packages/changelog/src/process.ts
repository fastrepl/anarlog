export function parseFrontmatter(content: string): {
  date: string | null;
  summary: string | null;
  body: string;
} {
  const trimmed = content.trim();
  const frontmatterMatch = trimmed.match(
    /^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/,
  );

  if (!frontmatterMatch) {
    return { date: null, summary: null, body: trimmed };
  }

  const frontmatterBlock = frontmatterMatch[1];
  const body = frontmatterMatch[2];

  const date = readFrontmatterValue(frontmatterBlock, "date");
  const summary = readFrontmatterValue(frontmatterBlock, "summary");

  return { date, summary, body };
}

function readFrontmatterValue(block: string, key: string) {
  const match = block.match(new RegExp(`^${key}:\\s*(.+)$`, "m"));
  if (!match) return null;

  const value = match[1].trim();
  const quote = value[0];
  if ((quote === `"` || quote === `'`) && value.endsWith(quote)) {
    return value.slice(1, -1);
  }

  return value;
}

export function fixImageUrls(content: string): string {
  return content.replace(
    /!\[([^\]]*)\]\(\/api\/assets\/([^)]+)\)/g,
    "![$1](https://auth.hyprnote.com/storage/v1/object/public/public_images/$2)",
  );
}

export function processContent(raw: string): {
  content: string;
  date: string | null;
  summary: string | null;
} {
  const { date, summary, body } = parseFrontmatter(raw);
  const markdown = fixImageUrls(body);
  return { content: markdown, date, summary };
}
