import { fetchWithCache, HOUR } from "@netlify/cache";
import { createServerFn } from "@tanstack/react-start";

import { env } from "../env";

const GITHUB_ORG_REPO = "fastrepl/anarlog";
const GITHUB_REPO_API_URL = `https://api.github.com/repos/${GITHUB_ORG_REPO}`;
const LAST_SEEN_STARS = 8355;

function getGitHubHeaders() {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "User-Agent": "Anarlog-Web",
    "X-GitHub-Api-Version": "2022-11-28",
  };

  if (env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${env.GITHUB_TOKEN}`;
  }

  return headers;
}

async function fetchGitHubStars() {
  try {
    const response = await fetchWithCache(
      GITHUB_REPO_API_URL,
      { headers: getGitHubHeaders() },
      { ttl: HOUR, durable: true },
    );

    if (!response.ok) {
      console.error("Failed to fetch GitHub repo stats:", response.status);
      return null;
    }

    const data = (await response.json()) as { stargazers_count?: unknown };
    return typeof data.stargazers_count === "number"
      ? data.stargazers_count
      : null;
  } catch (error) {
    console.error("Failed to fetch GitHub repo stats:", error);
    return null;
  }
}

export const getGitHubStars = createServerFn({ method: "GET" }).handler(
  async () => (await fetchGitHubStars()) ?? LAST_SEEN_STARS,
);
