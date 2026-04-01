import { useEffect, useState } from "react";
// @ts-ignore virtual module provided by ./vite.ts
import { latestContent, latestVersion } from "virtual:changelog";

import { processContent } from "@hypr/changelog";

export function getLatestVersion(): string | null {
  return latestVersion;
}

async function fetchChangelogFromGitHub(
  version: string,
): Promise<string | null> {
  const url = `https://raw.githubusercontent.com/fastrepl/char/main/packages/changelog/content/${version}.md`;
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
      if (version === latestVersion && latestContent) {
        const { content: parsed, date: parsedDate } =
          processContent(latestContent);
        setContent(parsed);
        setDate(parsedDate);
        setLoading(false);
        return;
      }

      const raw = await fetchChangelogFromGitHub(version);
      if (cancelled) return;

      if (raw) {
        const { content: parsed, date: parsedDate } = processContent(raw);
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
