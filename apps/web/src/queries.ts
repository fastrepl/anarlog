import { useQuery } from "@tanstack/react-query";

import { getGitHubStats, getStargazers } from "./functions/github";

const ORG_REPO = "fastrepl/char";
const LAST_SEEN_STARS = 7032;
const LAST_SEEN_FORKS = 432;

export function useGitHubStats() {
  return useQuery({
    queryKey: ["github-stats"],
    queryFn: async () => {
      const stats = await getGitHubStats();
      return {
        stars: stats.stars || LAST_SEEN_STARS,
        forks: stats.forks || LAST_SEEN_FORKS,
      };
    },
    staleTime: 1000 * 60 * 60,
  });
}

export interface Stargazer {
  username: string;
  avatar: string;
}

export function useGitHubStargazers() {
  return useQuery({
    queryKey: ["github-stargazers"],
    queryFn: () => getStargazers(),
    staleTime: 1000 * 60 * 60,
  });
}

export const GITHUB_ORG_REPO = ORG_REPO;
export const GITHUB_LAST_SEEN_STARS = LAST_SEEN_STARS;
export const GITHUB_LAST_SEEN_FORKS = LAST_SEEN_FORKS;
