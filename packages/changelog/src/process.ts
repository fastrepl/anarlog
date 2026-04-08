export function parseFrontmatter(content: string): {
  date: string | null;
  body: string;
} {
  const trimmed = content.trim();
  const frontmatterMatch = trimmed.match(
    /^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/,
  );

  if (!frontmatterMatch) {
    return { date: null, body: trimmed };
  }

  const frontmatterBlock = frontmatterMatch[1];
  const body = frontmatterMatch[2];

  const dateMatch = frontmatterBlock.match(/^date:\s*(.+)$/m);
  const date = dateMatch ? dateMatch[1].trim() : null;

  return { date, body };
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
} {
  const { date, body } = parseFrontmatter(raw);
  const markdown = fixImageUrls(body);
  return { content: markdown, date };
}
