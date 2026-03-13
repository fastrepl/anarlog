import { useEffect, useState } from "react";

export const changelogFiles = import.meta.glob(
  "../../../../../web/content/changelog/*.mdx",
  { query: "?raw", import: "default" },
);

export function getLatestVersion(): string | null {
  const versions = Object.keys(changelogFiles)
    .map((k) => {
      const match = k.match(/\/([^/]+)\.mdx$/);
      return match ? match[1] : null;
    })
    .filter((v): v is string => v !== null)
    .filter((v) => !v.includes("nightly"))
    .sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));

  return versions[0] || null;
}

function parseFrontmatter(content: string): {
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

function fixImageUrls(content: string): string {
  return content.replace(
    /!\[([^\]]*)\]\(\/api\/images\/([^)]+)\)/g,
    "![$1](https://auth.hyprnote.com/storage/v1/object/public/public_images/$2)",
  );
}

function processChangelogContent(raw: string): {
  content: string;
  date: string | null;
} {
  const { date, body } = parseFrontmatter(raw);
  const markdown = fixImageUrls(body);
  return { content: markdown, date };
}

async function fetchChangelogFromGitHub(
  version: string,
): Promise<string | null> {
  const url = `https://raw.githubusercontent.com/fastrepl/char/main/apps/web/content/changelog/${version}.mdx`;
  try {
    const response = await fetch(url);
    if (!response.ok) {
      return null;
    }
    return await response.text();
  } catch {
    return null;
  }
}

export function useChangelogContent(version: string) {
  const [content, setContent] = useState<string | null>(null);
  const [date, setDate] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadChangelog() {
      const key = Object.keys(changelogFiles).find((k) =>
        k.endsWith(`/${version}.mdx`),
      );

      if (key) {
        try {
          const raw = (await changelogFiles[key]()) as string;
          if (cancelled) return;
          const { content: parsed, date: parsedDate } =
            processChangelogContent(raw);
          setContent(parsed);
          setDate(parsedDate);
          setLoading(false);
          return;
        } catch {}
      }

      const raw = await fetchChangelogFromGitHub(version);
      if (cancelled) return;

      if (raw) {
        const { content: parsed, date: parsedDate } =
          processChangelogContent(raw);
        setContent(parsed);
        setDate(parsedDate);
      } else {
        setContent(null);
        setDate(null);
      }
      setLoading(false);
    }

    loadChangelog();

    return () => {
      cancelled = true;
    };
  }, [version]);

  return { content, date, loading };
}
