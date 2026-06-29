import { fetchWithCache, HOUR } from "@netlify/cache";
import { createServerFn } from "@tanstack/react-start";

import { env } from "../env";

const GITHUB_ORG_REPO = "fastrepl/anarlog";
const GITHUB_REPO_URL = `https://github.com/${GITHUB_ORG_REPO}`;
const GITHUB_REPO_API_URL = `https://api.github.com/repos/${GITHUB_ORG_REPO}`;
const CACHE_TTL = HOUR;

type GitHubStats = {
  stars: number | null;
  forks: number | null;
};

function getGitHubHeaders(accept = "application/vnd.github+json") {
  const headers: Record<string, string> = {
    Accept: accept,
    "User-Agent": "Anarlog-Web",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${env.GITHUB_TOKEN}`;
  }
  return headers;
}

async function fetchGitHub(url: string, accept?: string): Promise<Response> {
  return fetchWithCache(
    url,
    { headers: getGitHubHeaders(accept) },
    { ttl: CACHE_TTL, durable: true },
  );
}

function parseGitHubCounter(value: string | undefined) {
  if (!value) {
    return null;
  }

  const parsed = Number(value.replaceAll(",", "").trim());
  return Number.isFinite(parsed) ? parsed : null;
}

function extractGitHubCounter(html: string, patterns: RegExp[]) {
  for (const pattern of patterns) {
    const value = parseGitHubCounter(pattern.exec(html)?.[1]);
    if (value !== null) {
      return value;
    }
  }

  return null;
}

async function fetchGitHubStatsFromApi(): Promise<GitHubStats | null> {
  try {
    const response = await fetchGitHub(GITHUB_REPO_API_URL);

    if (!response.ok) {
      console.error("Failed to fetch GitHub repo stats:", response.status);
      return null;
    }

    const data = (await response.json()) as {
      stargazers_count?: unknown;
      forks_count?: unknown;
    };

    const stars =
      typeof data.stargazers_count === "number" ? data.stargazers_count : null;
    const forks =
      typeof data.forks_count === "number" ? data.forks_count : null;

    if (stars === null || forks === null) {
      console.error(
        "GitHub repo stats response did not include numeric counts",
      );
      return null;
    }

    return { stars, forks };
  } catch (error) {
    console.error("Failed to fetch GitHub repo stats from API:", error);
    return null;
  }
}

async function fetchGitHubStatsFromRepoPage(): Promise<GitHubStats | null> {
  try {
    const response = await fetchGitHub(
      GITHUB_REPO_URL,
      "text/html,application/xhtml+xml",
    );

    if (!response.ok) {
      console.error("Failed to fetch GitHub repo page:", response.status);
      return null;
    }

    const html = await response.text();
    const stars = extractGitHubCounter(html, [
      /id="repo-stars-counter-star"[^>]*title="([^"]+)"/,
      /id="repo-stars-counter-star"[^>]*aria-label="([0-9,]+)\s+users starred this repository"/,
    ]);
    const forks = extractGitHubCounter(html, [
      /id="repo-network-counter"[^>]*title="([^"]+)"/,
      /href="\/fastrepl\/anarlog\/forks"[\s\S]{0,250}?<strong>([0-9,]+)<\/strong>/,
    ]);

    if (stars === null || forks === null) {
      console.error("Failed to parse GitHub repo counts from repo page");
      return null;
    }

    return { stars, forks };
  } catch (error) {
    console.error("Failed to fetch GitHub repo stats from repo page:", error);
    return null;
  }
}

export const getGitHubStats = createServerFn({ method: "GET" }).handler(
  async () => {
    return (
      (await fetchGitHubStatsFromApi()) ??
      (await fetchGitHubStatsFromRepoPage()) ?? { stars: null, forks: null }
    );
  },
);

async function fetchStargazersFromRepoPage(count: number) {
  try {
    const response = await fetchGitHub(
      `${GITHUB_REPO_URL}/stargazers`,
      "text/html,application/xhtml+xml",
    );

    if (!response.ok) {
      console.error("Failed to fetch GitHub stargazers page:", response.status);
      return [];
    }

    const html = await response.text();
    const stargazers: { username: string; avatar: string }[] = [];
    const seen = new Set<string>();
    const pattern =
      /data-hovercard-url="\/users\/([^/"]+)\/hovercard"[\s\S]{0,300}?<img[^>]+src="([^"]+)"/g;

    for (const match of html.matchAll(pattern)) {
      const username = match[1];
      const avatar = match[2]?.replaceAll("&amp;", "&");

      if (!username || !avatar || seen.has(username)) {
        continue;
      }

      seen.add(username);
      stargazers.push({ username, avatar });

      if (stargazers.length >= count) {
        break;
      }
    }

    return stargazers;
  } catch (error) {
    console.error("Failed to parse GitHub stargazers page:", error);
    return [];
  }
}

export const getStargazers = createServerFn({ method: "GET" }).handler(
  async () => {
    const count = 42;

    try {
      if (!env.GITHUB_TOKEN) {
        const pageStargazers = await fetchStargazersFromRepoPage(count);
        if (pageStargazers.length > 0) {
          return pageStargazers;
        }
      }

      const repoStats = await fetchGitHubStatsFromApi();
      const totalStars = repoStats?.stars ?? 0;

      if (totalStars === 0) {
        return fetchStargazersFromRepoPage(count);
      }

      const perPage = 100;
      const numPages = Math.ceil(Math.min(count, totalStars) / perPage);
      const lastPage = Math.ceil(totalStars / perPage);
      const startPage = Math.max(1, lastPage - numPages + 1);

      const fetchPromises = [];
      for (let page = startPage; page <= lastPage; page++) {
        fetchPromises.push(
          fetchGitHub(
            `https://api.github.com/repos/${GITHUB_ORG_REPO}/stargazers?per_page=${perPage}&page=${page}`,
          ),
        );
      }

      const responses = await Promise.all(fetchPromises);
      const allStargazers: { username: string; avatar: string }[] = [];

      for (const response of responses) {
        if (!response.ok) continue;
        const data = await response.json();
        for (const user of data) {
          allStargazers.push({
            username: user.login,
            avatar: user.avatar_url,
          });
        }
      }

      const stargazers = allStargazers.reverse().slice(0, count);
      if (stargazers.length > 0) {
        return stargazers;
      }

      return fetchStargazersFromRepoPage(count);
    } catch {
      return fetchStargazersFromRepoPage(count);
    }
  },
);
