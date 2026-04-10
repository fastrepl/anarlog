import { fetchWithCache, HOUR } from "@netlify/cache";
import { createServerFn } from "@tanstack/react-start";

import { env } from "../env";

const GITHUB_ORG_REPO = "fastrepl/char";
const CACHE_TTL = HOUR;

function getGitHubHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github.v3+json",
    "User-Agent": "Char-Web",
  };
  if (env.GITHUB_TOKEN) {
    headers["Authorization"] = `token ${env.GITHUB_TOKEN}`;
  }
  return headers;
}

function getGitHubPageHeaders(): Record<string, string> {
  return {
    Accept: "text/html,application/xhtml+xml",
    "User-Agent": "Char-Web",
  };
}

async function fetchGitHub(
  url: string,
  headers = getGitHubHeaders(),
): Promise<Response> {
  return fetchWithCache(url, { headers }, { ttl: CACHE_TTL, durable: true });
}

function parseGitHubCount(value: string): number | null {
  const normalized = value.trim().toLowerCase().replace(/,/g, "");
  const match = normalized.match(/^([\d.]+)([km])?$/);

  if (!match) {
    const parsed = Number(normalized);
    return Number.isNaN(parsed) ? null : parsed;
  }

  const amount = Number(match[1]);

  if (Number.isNaN(amount)) {
    return null;
  }

  const multiplier = match[2] === "m" ? 1000000 : match[2] === "k" ? 1000 : 1;

  return Math.round(amount * multiplier);
}

function parseGitHubPageStat(
  html: string,
  path: "stargazers" | "forks",
): number | null {
  const escapedRepo = GITHUB_ORG_REPO.replace("/", "\\/");
  const match = html.match(
    new RegExp(
      `href="/${escapedRepo}/${path}"[\\s\\S]*?<strong>([^<]+)</strong>`,
      "i",
    ),
  );

  return match?.[1] ? parseGitHubCount(match[1]) : null;
}

async function getGitHubStatsFromApi(): Promise<{
  stars: number;
  forks: number;
}> {
  const response = await fetchGitHub(
    `https://api.github.com/repos/${GITHUB_ORG_REPO}`,
  );

  if (!response.ok) {
    throw new Error(`Failed to fetch repo info: ${response.status}`);
  }

  const data = await response.json();

  return {
    stars: data.stargazers_count ?? 0,
    forks: data.forks_count ?? 0,
  };
}

async function getGitHubStatsFromRepoPage(): Promise<{
  stars: number;
  forks: number;
}> {
  const response = await fetchGitHub(
    `https://github.com/${GITHUB_ORG_REPO}`,
    getGitHubPageHeaders(),
  );

  if (!response.ok) {
    throw new Error(`Failed to fetch GitHub repo page: ${response.status}`);
  }

  const html = await response.text();
  const stars = parseGitHubPageStat(html, "stargazers");
  const forks = parseGitHubPageStat(html, "forks");

  if (stars === null || forks === null) {
    throw new Error("Failed to parse GitHub repo page stats");
  }

  return { stars, forks };
}

export const getGitHubStats = createServerFn({ method: "GET" }).handler(
  async () => {
    try {
      return await getGitHubStatsFromApi();
    } catch (apiError) {
      try {
        return await getGitHubStatsFromRepoPage();
      } catch (pageError) {
        console.error("Failed to fetch GitHub stats", {
          apiError,
          pageError,
        });

        return { stars: 0, forks: 0 };
      }
    }
  },
);

export const getStargazers = createServerFn({ method: "GET" }).handler(
  async () => {
    try {
      const { stars: totalStars } = await getGitHubStatsFromApi();

      if (totalStars === 0) {
        return [];
      }

      const count = 512;
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

      return allStargazers.reverse().slice(0, count);
    } catch (error) {
      console.error("Failed to fetch GitHub stargazers", error);
      return [];
    }
  },
);
