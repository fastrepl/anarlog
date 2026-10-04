import { processContent } from "@anlg/changelog/process";

import type { ChangelogPublication } from "../../changelog-publication";
import { getChangelogReleaseFromPath } from "./changelog-path.ts";

export function createChangelogEntries(
  rawEntries: Record<
    string,
    { raw: string; publication: ChangelogPublication | null }
  >,
) {
  return Object.entries(rawEntries)
    .flatMap(([filePath, { raw, publication }]) => {
      const release = getChangelogReleaseFromPath(filePath);
      if (!release) return [];
      const { content, date, summary } = processContent(raw);
      return [
        {
          ...release,
          content,
          date: publication
            ? new Date(publication.publishedAt).toISOString().slice(0, 10)
            : date,
          publishedAt: publication?.publishedAt ?? date,
          summary,
          channel: publication?.channel ?? null,
          availability: publication?.availability ?? [],
        },
      ];
    })
    .sort((a, b) => {
      const dateDifference =
        Date.parse(b.publishedAt ?? "") - Date.parse(a.publishedAt ?? "");
      if (Number.isFinite(dateDifference) && dateDifference !== 0)
        return dateDifference;
      return (
        a.stream.localeCompare(b.stream) ||
        b.version.localeCompare(a.version, undefined, { numeric: true })
      );
    });
}

export function availabilityLabel(
  destination: ChangelogPublication["availability"][number]["destination"],
) {
  return {
    testflight: "TestFlight",
    "app-store": "App Store",
    "play-internal": "Google Play internal testing",
    "play-beta": "Google Play beta",
    "play-production": "Google Play",
  }[destination];
}
