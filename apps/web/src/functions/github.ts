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

async function fetchGitHub(url: string): Promise<Response> {
  return fetchWithCache(
    url,
    { headers: getGitHubHeaders() },
    { ttl: CACHE_TTL, durable: true },
  );
}

export const getGitHubStats = createServerFn({ method: "GET" }).handler(
  async () => {
    try {
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
    } catch {
      return { stars: 0, forks: 0 };
    }
  },
);

export const getStargazers = createServerFn({ method: "GET" }).handler(
  async () => {
    try {
      const repoResponse = await fetchGitHub(
        `https://api.github.com/repos/${GITHUB_ORG_REPO}`,
      );

      if (!repoResponse.ok) {
        throw new Error(`Failed to fetch repo info: ${repoResponse.status}`);
      }

      const repoData = await repoResponse.json();
      const totalStars = repoData.stargazers_count ?? 0;

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
    } catch {
      return [];
    }
  },
);
