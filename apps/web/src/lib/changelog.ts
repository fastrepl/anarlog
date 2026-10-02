import rawEntries from "virtual:published-changelogs";

import { createChangelogEntries } from "./changelog-data";
import type { ChangelogStream } from "./changelog-path";

export const changelogEntries = createChangelogEntries(rawEntries);

export function getChangelogEntry(stream: ChangelogStream, version: string) {
  return changelogEntries.find(
    (entry) => entry.stream === stream && entry.version === version,
  );
}

export function formatChangelogDate(date: string) {
  const parsed = new Date(`${date}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) return date;
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(parsed);
}
