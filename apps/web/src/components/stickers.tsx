import { cn } from "@hypr/utils";

import {
  GITHUB_LAST_SEEN_STARS,
  GITHUB_ORG_REPO,
  useGitHubStats,
} from "../queries";

export function YCombinatorSticker({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "surface z-10 rounded-xl p-2 shadow-sm transition-transform hover:scale-105",
        className,
      )}
    >
      <a
        href="https://www.ycombinator.com/companies/char"
        target="_blank"
        rel="noopener noreferrer"
        className="surface shadow-ring flex items-center gap-3 rounded-md py-2 pr-4 pl-2"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 18 18"
          className="size-10 shrink-0"
        >
          <path d="M 0 18 L 18 18 L 18 0 L 0 0 Z" fill="rgb(251,101,30)" />
          <path
            d="M 9.731 9.894 L 9.731 13.894 L 8.212 13.894 L 8.212 9.894 L 4.337 4.106 L 6.187 4.106 L 8.977 8.381 L 11.756 4.106 L 13.607 4.106 Z"
            fill="rgb(255,255,255)"
          />
        </svg>
        <div className="flex flex-col">
          <span className="text-xs text-stone-400">Backed by</span>
          <span className="text-sm font-semibold text-stone-700">
            Y Combinator
          </span>
        </div>
      </a>
    </div>
  );
}

function formatStars(n: number) {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`;
}

export function GitHubStarsSticker({ className }: { className?: string }) {
  const githubStats = useGitHubStats();
  const starCount = githubStats.data?.stars ?? GITHUB_LAST_SEEN_STARS;

  return (
    <div
      className={cn(
        "group surface relative cursor-pointer rounded-full p-1 shadow-sm transition-transform hover:scale-105 active:scale-[96%]",
        className,
      )}
    >
      <div className="absolute top-0 left-1/2 h-full w-1/5 -translate-x-1/3 bg-white/20 blur-sm transition-[width,transform,background-color] duration-200 group-hover:w-5/5 group-hover:-translate-x-1/2 group-hover:bg-white/10"></div>
      <a
        href={`https://github.com/${GITHUB_ORG_REPO}`}
        target="_blank"
        rel="noopener noreferrer"
        className="z-10 inline-flex items-center gap-2.5 rounded-full bg-stone-900 py-2 pr-10 pl-2 text-white"
      >
        <svg
          viewBox="0 0 24 24"
          className="size-8 shrink-0 fill-white"
          aria-hidden="true"
        >
          <path d="M12 2A10 10 0 0 0 2 12c0 4.42 2.87 8.17 6.84 9.5.5.08.66-.23.66-.5v-1.69c-2.77.6-3.36-1.34-3.36-1.34-.46-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.87 1.52 2.34 1.07 2.91.83.09-.65.35-1.09.63-1.34-2.22-.25-4.55-1.11-4.55-4.92 0-1.11.38-2 1.03-2.71-.1-.25-.45-1.29.1-2.64 0 0 .84-.27 2.75 1.02.79-.22 1.65-.33 2.5-.33.85 0 1.71.11 2.5.33 1.91-1.29 2.75-1.02 2.75-1.02.55 1.35.2 2.39.1 2.64.65.71 1.03 1.6 1.03 2.71 0 3.82-2.34 4.66-4.57 4.91.36.31.69.92.69 1.85V21c0 .27.16.59.67.5C19.14 20.16 22 16.42 22 12A10 10 0 0 0 12 2z" />
        </svg>
        <div className="flex flex-col gap-1">
          <span className="text-sm font-bold tracking-tight tabular-nums">
            {formatStars(starCount)} Stars
          </span>
          <span className="text-xs text-stone-400">on GitHub</span>
        </div>
      </a>
    </div>
  );
}
