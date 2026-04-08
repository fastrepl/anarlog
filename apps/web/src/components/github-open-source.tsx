import { Icon } from "@iconify-icon/react";
import { Link } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { useMountEffect } from "@/hooks/useMountEffect";

import {
  GITHUB_LAST_SEEN_FORKS,
  GITHUB_LAST_SEEN_STARS,
  GITHUB_ORG_REPO,
  useGitHubStargazers,
  useGitHubStats,
} from "../queries";

const AVATAR_SIZE = 40;
const AVATAR_GAP = 4;
const DEFAULT_DISPLAY_COUNT = 50;
const DEFAULT_ROTATION_INTERVAL_MS = 1000;
const WAVE_ROTATION_INTERVAL_MS = 120;
const WAVE_REPEAT_DELAY_MS = 1200;
const AVATAR_SWAP_TRANSITION = {
  duration: 0.24,
  ease: [0.22, 1, 0.36, 1],
} as const;

function StatBadge({
  type,
  count,
}: {
  type: "stars" | "forks";
  count: number;
}) {
  const renderCount = (n: number) =>
    n > 1000 ? `${(n / 1000).toFixed(1)}k` : n;

  return (
    <div className="border-color-brand surface-subtle flex min-w-[100px] flex-col items-end justify-end gap-4 rounded-md border px-4 py-4">
      <p className="text-fg font-mono text-sm tracking-wide uppercase opacity-50">
        {type === "stars" ? "Stars" : "Forks"}
      </p>
      <h3 className="text-fg text-right font-mono text-2xl font-medium">
        {renderCount(count)}
      </h3>
    </div>
  );
}

function Avatar({ username, avatar }: { username: string; avatar: string }) {
  return (
    <div className="group relative size-[40px] shrink-0 overflow-hidden rounded-xs border border-neutral-200 bg-neutral-100 transition-colors hover:border-neutral-400">
      <AnimatePresence initial={false}>
        <motion.a
          key={`${username}-${avatar}`}
          href={`https://github.com/${username}`}
          target="_blank"
          rel="noopener noreferrer"
          initial={{
            opacity: 0.5,
            x: 4,
            scale: 0.985,
          }}
          animate={{
            opacity: 1,
            x: 0,
            scale: 1,
          }}
          exit={{
            opacity: 0.35,
            x: -4,
            scale: 1.015,
          }}
          transition={AVATAR_SWAP_TRANSITION}
          className="absolute inset-0 block cursor-pointer"
        >
          <img
            src={avatar}
            alt={`${username}'s avatar`}
            className="h-full w-full object-cover transition-transform duration-200 group-hover:scale-105"
          />
        </motion.a>
      </AnimatePresence>
    </div>
  );
}

function getAvatarColumnCount(width: number) {
  return Math.max(
    1,
    Math.floor((width + AVATAR_GAP) / (AVATAR_SIZE + AVATAR_GAP)),
  );
}

function getPackedDisplayCount({
  columnCount,
  profileCount,
  rows,
}: {
  columnCount: number;
  profileCount: number;
  rows: number;
}) {
  if (profileCount <= columnCount) {
    return profileCount;
  }

  const maxVisible = Math.min(profileCount, columnCount * rows);
  const fullRows = Math.floor(maxVisible / columnCount);

  return fullRows > 0 ? fullRows * columnCount : maxVisible;
}

function RotatingAvatarSet({
  profiles,
  displayCount,
  columnCount,
}: {
  profiles: { username: string; avatar: string }[];
  displayCount: number;
  columnCount?: number | null;
}) {
  const [visible, setVisible] = useState(() => profiles.slice(0, displayCount));
  const poolRef = useRef(displayCount);
  const waveSlotRef = useRef(0);

  const pickNext = useCallback(() => {
    const idx = poolRef.current % profiles.length;
    poolRef.current = idx + 1;
    return profiles[idx];
  }, [profiles]);

  useEffect(() => {
    if (profiles.length <= displayCount) return;

    const isPackedRows =
      Math.max(1, columnCount ?? displayCount) < displayCount;
    const stepDelayMs = isPackedRows
      ? WAVE_ROTATION_INTERVAL_MS
      : DEFAULT_ROTATION_INTERVAL_MS;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;

    const tick = () => {
      const idx = waveSlotRef.current % displayCount;
      waveSlotRef.current = idx + 1;
      const nextProfile = pickNext();

      setVisible((prev) => {
        const next = [...prev];
        next[idx] = nextProfile;
        return next;
      });

      const completedSweep = waveSlotRef.current % displayCount === 0;
      const nextDelay =
        isPackedRows && completedSweep ? WAVE_REPEAT_DELAY_MS : stepDelayMs;

      timeoutId = setTimeout(tick, nextDelay);
    };

    timeoutId = setTimeout(tick, stepDelayMs);

    return () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [columnCount, displayCount, profiles.length, pickNext]);

  return (
    <>
      {visible.map((profile, i) => (
        <Avatar key={i} username={profile.username} avatar={profile.avatar} />
      ))}
    </>
  );
}

export function RotatingAvatarGrid({
  profiles,
  rows,
}: {
  profiles: { username: string; avatar: string }[];
  rows?: number;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [columnCount, setColumnCount] = useState<number | null>(null);

  useMountEffect(() => {
    if (!rows) {
      return;
    }

    const el = containerRef.current;
    if (!el) {
      return;
    }

    const updateColumnCount = (width: number) => {
      setColumnCount(getAvatarColumnCount(width));
    };

    updateColumnCount(el.clientWidth);

    const observer = new ResizeObserver(([entry]) => {
      updateColumnCount(entry.contentRect.width);
    });

    observer.observe(el);
    return () => observer.disconnect();
  });

  const displayCount =
    rows && columnCount
      ? getPackedDisplayCount({
          columnCount,
          profileCount: profiles.length,
          rows,
        })
      : Math.min(profiles.length, DEFAULT_DISPLAY_COUNT);

  const resetKey = `${displayCount}-${profiles.length}-${profiles[0]?.username ?? ""}`;

  return (
    <div ref={containerRef} className="mt-12 flex flex-wrap gap-1">
      <RotatingAvatarSet
        key={resetKey}
        profiles={profiles}
        displayCount={displayCount}
        columnCount={columnCount}
      />
    </div>
  );
}

export function GitHubOpenSource() {
  const githubStats = useGitHubStats();
  const { data: stargazers = [] } = useGitHubStargazers();

  const STARS_COUNT = githubStats.data?.stars ?? GITHUB_LAST_SEEN_STARS;
  const FORKS_COUNT = githubStats.data?.forks ?? GITHUB_LAST_SEEN_FORKS;

  return (
    <section id="opensource">
      <div className="px-4 py-16">
        <h2 className="text-fg border-color-brand mb-8 border-b pb-8 font-mono text-2xl tracking-wide md:text-4xl">
          Open source
        </h2>
        <div
          className={cn([
            "flex flex-col items-center gap-6",
            "md:flex-row md:items-center md:justify-between md:gap-12",
          ])}
        >
          <div className="flex flex-col items-center gap-4 md:items-start">
            <p className="text-fg-muted max-w-md text-center text-base leading-relaxed md:text-left">
              Char values privacy and community, so it's been transparent from
              day one.
            </p>
            <div
              className={cn([
                "flex flex-col items-center gap-3",
                "sm:flex-row md:items-start",
              ])}
            >
              <a
                href={`https://github.com/${GITHUB_ORG_REPO}`}
                target="_blank"
                rel="noopener noreferrer"
                className={cn([
                  "group inline-flex h-14 items-center justify-center gap-2 px-8",
                  "text-fg border-color-brand rounded-full border",
                  "hover:scale-[102%] hover:bg-[var(--color-brand-dark)] hover:text-white active:scale-[98%]",
                  "cursor-pointer transition-all",
                ])}
              >
                <Icon icon="mdi:github" className="text-lg" />
                View on GitHub
              </a>
              <Link
                to="/opensource/"
                className={cn([
                  "inline-flex h-14 items-center justify-center px-8",
                  "text-fg border-color-brand rounded-full border",
                  "hover:scale-[102%] hover:bg-[var(--color-brand-dark)] hover:text-white active:scale-[98%]",
                  "cursor-pointer transition-all",
                ])}
              >
                Why Open source?
              </Link>
            </div>
          </div>
          <div className="flex shrink-0 gap-2">
            <StatBadge type="stars" count={STARS_COUNT} />
            <StatBadge type="forks" count={FORKS_COUNT} />
          </div>
        </div>

        {stargazers.length > 0 ? (
          <RotatingAvatarGrid profiles={stargazers} rows={2} />
        ) : null}
      </div>
    </section>
  );
}
