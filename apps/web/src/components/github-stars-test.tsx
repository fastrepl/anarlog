import { cn } from "@hypr/utils";

import {
  GITHUB_LAST_SEEN_STARS,
  GITHUB_ORG_REPO,
  useGitHubStats,
} from "../queries";

function GitHubIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className}>
      <path d="M12 2A10 10 0 0 0 2 12c0 4.42 2.87 8.17 6.84 9.5.5.08.66-.23.66-.5v-1.69c-2.77.6-3.36-1.34-3.36-1.34-.46-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.87 1.52 2.34 1.07 2.91.83.09-.65.35-1.09.63-1.34-2.22-.25-4.55-1.11-4.55-4.92 0-1.11.38-2 1.03-2.71-.1-.25-.45-1.29.1-2.64 0 0 .84-.27 2.75 1.02.79-.22 1.65-.33 2.5-.33.85 0 1.71.11 2.5.33 1.91-1.29 2.75-1.02 2.75-1.02.55 1.35.2 2.39.1 2.64.65.71 1.03 1.6 1.03 2.71 0 3.82-2.34 4.66-4.57 4.91.36.31.69.92.69 1.85V21c0 .27.16.59.67.5C19.14 20.16 22 16.42 22 12A10 10 0 0 0 12 2z" />
    </svg>
  );
}

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
        <GitHubIcon className="size-5" />
        <span className="ml-2">{render(starCount)} stars</span>
      </button>
    </a>
  );
}
