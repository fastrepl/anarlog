import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { allRoadmaps } from "content-collections";
import { useRef } from "react";

import { cn } from "@hypr/utils";

import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";
import { Image } from "@/components/image";
import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";

export const Route = createFileRoute("/_view/roadmap/")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Roadmap - Char" },
      {
        name: "description",
        content:
          "See what we're building next for Char. Our product roadmap and future plans.",
      },
    ],
  }),
});

type RoadmapStatus = "done" | "in-progress" | "todo";

type RoadmapPriority = "high" | "mid" | "low";

type RoadmapItem = {
  slug: string;
  title: string;
  status: RoadmapStatus;
  labels: string[];
  githubIssues: string[];
  mdx: string;
  priority: RoadmapPriority;
  date: string;
  description: string;
};

const priorityOrder: Record<RoadmapPriority, number> = {
  high: 1,
  mid: 2,
  low: 3,
};

const statusOrder: Record<RoadmapStatus, number> = {
  "in-progress": 1,
  todo: 2,
  done: 3,
};

function getRoadmapItems(): RoadmapItem[] {
  const items = allRoadmaps.map((item) => ({
    slug: item.slug,
    title: item.title,
    status: item.status,
    labels: item.labels || [],
    githubIssues: item.githubIssues || [],
    mdx: item.mdx,
    priority: item.priority,
    date: item.date,
    description: item.content.trim(),
  }));

  return items.sort((a, b) => {
    const statusDiff = statusOrder[a.status] - statusOrder[b.status];
    if (statusDiff !== 0) return statusDiff;
    return priorityOrder[a.priority] - priorityOrder[b.priority];
  });
}

function Component() {
  const items = getRoadmapItems();
  const heroInputRef = useRef<HTMLInputElement>(null);

  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <div className="px-6 py-12 lg:py-20">
          <header className="mb-12 text-center">
            <h1 className="mb-6 font-serif text-4xl text-stone-600 sm:text-5xl lg:text-6xl">
              Product Roadmap
            </h1>
            <p className="mx-auto max-w-2xl text-xl text-neutral-600">
              See what we're building and what's coming next. We're always
              listening to feedback from our community.
            </p>
          </header>

          <TableView items={items} />

          <CTASection heroInputRef={heroInputRef} />
        </div>
      </div>
    </div>
  );
}

const priorityConfig: Record<
  RoadmapPriority,
  { label: string; className: string }
> = {
  high: {
    label: "High",
    className: "bg-linear-to-t from-red-200 to-red-100 text-red-900",
  },
  mid: {
    label: "Mid",
    className: "bg-linear-to-t from-orange-200 to-orange-100 text-orange-900",
  },
  low: {
    label: "Low",
    className:
      "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900",
  },
};

const statusConfig: Record<
  RoadmapStatus,
  { label: string; icon: string; className: string }
> = {
  "in-progress": {
    label: "In Progress",
    icon: "mdi:progress-clock",
    className: "bg-linear-to-b from-[#03BCF1] to-[#127FE5] text-white",
  },
  todo: {
    label: "To Do",
    icon: "mdi:calendar-clock",
    className:
      "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900",
  },
  done: {
    label: "Done",
    icon: "mdi:check-circle",
    className: "bg-linear-to-t from-green-200 to-green-100 text-green-900",
  },
};

function TableView({ items }: { items: RoadmapItem[] }) {
  return (
    <div className="-mx-6 overflow-x-auto">
      <table className="w-full border-collapse">
        <thead>
          <tr className="bg-neutral-50">
            <th className="border-y border-neutral-100 px-3 py-2 text-left text-sm font-medium whitespace-nowrap text-stone-500">
              Name
            </th>
            <th className="border-y border-l border-neutral-100 px-3 py-2 text-left text-sm font-medium whitespace-nowrap text-stone-500">
              Status
            </th>
            <th className="border-y border-l border-neutral-100 px-3 py-2 text-left text-sm font-medium whitespace-nowrap text-stone-500">
              Priority
            </th>
            <th className="border-y border-l border-neutral-100 px-3 py-2 text-left text-sm font-medium whitespace-nowrap text-stone-500">
              Date
            </th>
          </tr>
        </thead>
        <tbody>
          {items.map((item) => {
            const priorityInfo = priorityConfig[item.priority];
            const statusInfo = statusConfig[item.status];

            return (
              <tr
                key={item.slug}
                className="transition-colors hover:bg-stone-50"
              >
                <td className="border-y border-neutral-100 px-3 py-2 whitespace-nowrap">
                  <Link
                    to="/roadmap/$slug/"
                    params={{ slug: item.slug }}
                    className="font-medium text-stone-700 hover:text-stone-900 hover:underline"
                  >
                    {item.title}
                  </Link>
                </td>
                <td className="border-y border-l border-neutral-100 px-3 py-2">
                  <span
                    className={cn([
                      "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap",
                      statusInfo.className,
                    ])}
                  >
                    <Icon icon={statusInfo.icon} />
                    {statusInfo.label}
                  </span>
                </td>
                <td className="border-y border-l border-neutral-100 px-3 py-2">
                  <span
                    className={cn([
                      "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap",
                      priorityInfo.className,
                    ])}
                  >
                    {priorityInfo.label}
                  </span>
                </td>
                <td className="border-y border-l border-neutral-100 px-3 py-2 text-sm whitespace-nowrap text-stone-500">
                  {item.date || "—"}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function CTASection({
  heroInputRef,
}: {
  heroInputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);

  const getButtonLabel = () => {
    if (platform === "mobile") {
      return "Get reminder";
    }
    return platformCTA.label;
  };

  const handleCTAClick = () => {
    if (platformCTA.action === "waitlist") {
      window.scrollTo({ top: 0, behavior: "smooth" });
      setTimeout(() => {
        if (heroInputRef.current) {
          heroInputRef.current.focus();
          heroInputRef.current.parentElement?.classList.add(
            "animate-shake",
            "border-stone-600",
          );
          setTimeout(() => {
            heroInputRef.current?.parentElement?.classList.remove(
              "animate-shake",
              "border-stone-600",
            );
          }, 500);
        }
      }, 500);
    }
  };

  return (
    <section className="-mx-6 mt-16 bg-linear-to-t from-stone-50/30 to-stone-100/30 px-6 py-16">
      <div className="flex flex-col items-center gap-6 text-center">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <Image
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-serif text-2xl sm:text-3xl">
          Where conversations
          <br className="sm:hidden" /> stay yours
        </h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          Start using Char today and bring clarity to your back-to-back meetings
        </p>
        <div className="flex flex-col items-center justify-center gap-4 pt-6 sm:flex-row">
          {platformCTA.action === "download" ? (
            <DownloadButton />
          ) : (
            <button
              onClick={handleCTAClick}
              className={cn([
                "group flex h-12 items-center justify-center px-6 text-base sm:text-lg",
                "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
                "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
                "transition-all",
              ])}
            >
              {getButtonLabel()}
              <svg
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                strokeWidth="1.5"
                stroke="currentColor"
                className="ml-2 h-5 w-5 transition-transform group-hover:translate-x-1"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="m12.75 15 3-3m0 0-3-3m3 3h-7.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
                />
              </svg>
            </button>
          )}
          <div className="hidden sm:block">
            <GithubStars />
          </div>
        </div>
      </div>
    </section>
  );
}
