import { processContent } from "@hypr/changelog";

const rawEntries = import.meta.glob(
  "../../../../packages/changelog/content/*.md",
  {
    eager: true,
    import: "default",
    query: "?raw",
  },
) as Record<string, string>;

export const changelogEntries = Object.entries(rawEntries)
  .map(([filePath, raw]) => {
    const version = filePath.split("/").pop()?.replace(/\.md$/, "") ?? "";
    const { content, date, summary } = processContent(raw);

    return {
      version,
      content,
      date: normalizeDate(date),
      summary,
    };
  })
  .filter((entry) => entry.version)
  .sort((a, b) => compareVersionsDesc(a.version, b.version));

export function getChangelogEntry(version: string) {
  return changelogEntries.find((entry) => entry.version === version);
}

function normalizeDate(date: string | null) {
  return date?.replace(/^["']|["']$/g, "") ?? null;
}

function compareVersionsDesc(a: string, b: string) {
  const left = a.split(".").map(Number);
  const right = b.split(".").map(Number);
  const length = Math.max(left.length, right.length);

  for (let i = 0; i < length; i++) {
    const diff = (right[i] ?? 0) - (left[i] ?? 0);
    if (diff !== 0) return diff;
  }

  return b.localeCompare(a);
}

export function formatChangelogDate(date: string) {
  const parsed = new Date(`${date}T00:00:00Z`);

  if (Number.isNaN(parsed.getTime())) {
    return date;
  }

  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(parsed);
}
