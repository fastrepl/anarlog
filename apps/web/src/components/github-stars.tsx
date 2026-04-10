import { Icon } from "@iconify-icon/react";

import { cn } from "@hypr/utils";

import {
  GITHUB_LAST_SEEN_STARS,
  GITHUB_ORG_REPO,
  useGitHubStats,
} from "../queries";

export function GithubStars() {
  const githubStats = useGitHubStats();
  const starCount = githubStats.data?.stars ?? GITHUB_LAST_SEEN_STARS;
  const render = (n: number) => (n > 1000 ? `${(n / 1000).toFixed(1)}k` : n);

  return (
    <a href={`https://github.com/${GITHUB_ORG_REPO}`} target="_blank">
      <button
        className={cn([
          "group flex h-14 items-center justify-center px-8 text-base sm:text-lg",
          "border-color-bright text-fg rounded-full border",
          "hover:scale-[102%] hover:bg-[var(--color-brand-dark)] hover:text-white active:scale-[98%]",
          "cursor-pointer transition-all",
        ])}
      >
        <Icon icon="mdi:github" className="text-xl" />
        <span className="ml-2">{render(starCount)} stars</span>
      </button>
    </a>
  );
}
