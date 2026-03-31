import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { allRoadmaps } from "content-collections";

import { cn } from "@hypr/utils";

import { CTASection } from "@/components/cta-section";

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

  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <div className="px-4 py-16 lg:py-24">
          <header className="mb-12 text-left">
            <h1 className="text-color mb-6 font-mono text-4xl tracking-tight sm:text-5xl">
              Product Roadmap
            </h1>
            <p className="text-fg-muted mx-auto max-w-2xl text-lg sm:text-xl">
              See what we're building and what's coming next. We're always
              listening to feedback from our community.
            </p>
          </header>

          <TableView items={items} />

          <CTASection />
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
    <div className="-mx-4 overflow-x-auto">
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
