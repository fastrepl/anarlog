import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  DownloadIcon,
  ExternalLinkIcon,
  RefreshCwIcon,
  SearchIcon,
  SparklesIcon,
  StarIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import type { StarLead } from "@/functions/github-stars";

export const Route = createFileRoute("/admin/stars/")({
  component: StarsPage,
});

function StarsPage() {
  const queryClient = useQueryClient();
  const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const [filter, setFilter] = useState<"all" | "researched" | "unresearched">(
    "all",
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ["starLeads", filter],
    queryFn: async () => {
      const params = new URLSearchParams();
      params.set("limit", "200");
      if (filter === "researched") params.set("researched", "true");
      const response = await fetch(`/api/admin/stars/leads?${params}`);
      if (!response.ok) throw new Error("Failed to fetch leads");
      return response.json() as Promise<{ leads: StarLead[]; total: number }>;
    },
  });

  const fetchMutation = useMutation({
    mutationFn: async (source: "stargazers" | "activity") => {
      const response = await fetch("/api/admin/stars/fetch", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ source }),
      });
      if (!response.ok) throw new Error("Failed to fetch stars");
      return response.json() as Promise<{ added: number; total: number }>;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["starLeads"] });
    },
  });

  const researchMutation = useMutation({
    mutationFn: async (username: string) => {
      const response = await fetch("/api/admin/stars/research", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username }),
      });
      if (!response.ok) {
        const err = await response.json();
        throw new Error((err as { error?: string }).error || "Research failed");
      }
      return response.json() as Promise<{
        success: boolean;
        lead?: StarLead;
      }>;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["starLeads"] });
    },
  });

  const autoFetchedRef = useRef(false);
  useEffect(() => {
    if (
      !autoFetchedRef.current &&
      !isLoading &&
      data &&
      data.total === 0 &&
      filter === "all"
    ) {
      autoFetchedRef.current = true;
      fetchMutation.mutate("stargazers");
    }
  }, [isLoading, data, filter]);

  const leads = data?.leads ?? [];
  const total = data?.total ?? 0;

  const filteredLeads = leads.filter((lead) => {
    if (filter === "unresearched" && lead.researched_at) return false;
    if (!searchQuery) return true;
    const q = searchQuery.toLowerCase();
    return (
      lead.github_username.toLowerCase().includes(q) ||
      (lead.name && lead.name.toLowerCase().includes(q)) ||
      (lead.company && lead.company.toLowerCase().includes(q))
    );
  });

  const toggleRow = (username: string) => {
    setExpandedRows((prev) => {
      const next = new Set(prev);
      if (next.has(username)) {
        next.delete(username);
      } else {
        next.add(username);
      }
      return next;
    });
  };

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-200 bg-white px-6 py-4">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h1 className="text-lg font-semibold text-neutral-900">
              GitHub Stars
            </h1>
            <p className="mt-0.5 text-sm text-neutral-500">
              Track and qualify leads from GitHub activity on fastrepl/char
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => fetchMutation.mutate("stargazers")}
              disabled={fetchMutation.isPending}
              className="flex items-center gap-1.5 rounded-lg border border-neutral-200 bg-white px-3 py-1.5 text-sm transition-colors hover:bg-neutral-50 disabled:opacity-50"
            >
              {fetchMutation.isPending ? (
                <Spinner size={14} />
              ) : (
                <StarIcon className="h-3.5 w-3.5" />
              )}
              Fetch Stars
            </button>
            <button
              type="button"
              onClick={() => fetchMutation.mutate("activity")}
              disabled={fetchMutation.isPending}
              className="flex items-center gap-1.5 rounded-lg border border-neutral-200 bg-white px-3 py-1.5 text-sm transition-colors hover:bg-neutral-50 disabled:opacity-50"
            >
              {fetchMutation.isPending ? (
                <Spinner size={14} />
              ) : (
                <DownloadIcon className="h-3.5 w-3.5" />
              )}
              Fetch Activity
            </button>
            <button
              type="button"
              onClick={() => refetch()}
              disabled={isLoading}
              className="flex items-center gap-1.5 rounded-lg border border-neutral-200 bg-white px-3 py-1.5 text-sm transition-colors hover:bg-neutral-50 disabled:opacity-50"
            >
              <RefreshCwIcon
                className={cn("h-3.5 w-3.5", isLoading && "animate-spin")}
              />
              Refresh
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="relative max-w-sm flex-1">
            <SearchIcon className="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-neutral-400" />
            <input
              type="text"
              placeholder="Search by username, name, or company..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full rounded-lg border border-neutral-200 py-1.5 pr-3 pl-9 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
            />
          </div>

          <div className="flex items-center gap-1 rounded-lg bg-neutral-100 p-0.5">
            {(["all", "researched", "unresearched"] as const).map((f) => (
              <button
                type="button"
                key={f}
                onClick={() => setFilter(f)}
                className={cn(
                  "rounded-md px-3 py-1 text-xs font-medium capitalize transition-colors",
                  filter === f
                    ? "bg-white text-neutral-900 shadow-sm"
                    : "text-neutral-500 hover:text-neutral-700",
                )}
              >
                {f}
              </button>
            ))}
          </div>

          <span className="text-xs text-neutral-500">
            {filteredLeads.length} of {total} leads
          </span>
        </div>

        {fetchMutation.isSuccess && (
          <div className="mt-2 rounded-lg border border-green-200 bg-green-50 px-3 py-1.5 text-sm text-green-700">
            Fetched {fetchMutation.data.added} new entries. Total:{" "}
            {fetchMutation.data.total}
          </div>
        )}
        {fetchMutation.isError && (
          <div className="mt-2 rounded-lg border border-red-200 bg-red-50 px-3 py-1.5 text-sm text-red-700">
            Error: {fetchMutation.error.message}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-auto">
        {isLoading ? (
          <div className="flex h-64 items-center justify-center">
            <Spinner size={24} />
          </div>
        ) : filteredLeads.length === 0 ? (
          <div className="flex h-64 flex-col items-center justify-center text-neutral-400">
            <StarIcon className="mb-3 h-12 w-12 stroke-1" />
            <p className="text-sm">No leads found</p>
            <p className="mt-1 text-xs">
              Click "Fetch Stars" to pull stargazers from GitHub
            </p>
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-neutral-50">
              <tr className="border-b border-neutral-200">
                <th className="w-8 px-3 py-2" />
                <th className="px-3 py-2 text-left font-medium text-neutral-600">
                  User
                </th>
                <th className="px-3 py-2 text-left font-medium text-neutral-600">
                  Event
                </th>
                <th className="px-3 py-2 text-left font-medium text-neutral-600">
                  Score
                </th>
                <th className="px-3 py-2 text-left font-medium text-neutral-600">
                  Match
                </th>
                <th className="px-3 py-2 text-left font-medium text-neutral-600">
                  Company
                </th>
                <th className="px-3 py-2 text-left font-medium text-neutral-600">
                  Status
                </th>
                <th className="px-3 py-2 text-right font-medium text-neutral-600">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {filteredLeads.map((lead) => (
                <LeadRow
                  key={lead.github_username}
                  lead={lead}
                  isExpanded={expandedRows.has(lead.github_username)}
                  onToggle={() => toggleRow(lead.github_username)}
                  onResearch={() =>
                    researchMutation.mutate(lead.github_username)
                  }
                  isResearching={
                    researchMutation.isPending &&
                    researchMutation.variables === lead.github_username
                  }
                />
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function ScoreBadge({ score }: { score: number | null }) {
  if (score === null || score === undefined) {
    return (
      <span className="rounded-full bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-400">
        --
      </span>
    );
  }

  let color = "bg-neutral-100 text-neutral-600";
  if (score >= 80) color = "bg-green-100 text-green-700";
  else if (score >= 60) color = "bg-blue-100 text-blue-700";
  else if (score >= 40) color = "bg-yellow-100 text-yellow-700";
  else if (score >= 20) color = "bg-orange-100 text-orange-700";
  else color = "bg-red-100 text-red-600";

  return (
    <span
      className={cn("rounded-full px-2 py-0.5 text-xs font-semibold", color)}
    >
      {score}
    </span>
  );
}

function EventBadge({ type }: { type: string }) {
  const colors: Record<string, string> = {
    star: "bg-yellow-50 text-yellow-700 border-yellow-200",
    fork: "bg-purple-50 text-purple-700 border-purple-200",
    issue: "bg-green-50 text-green-700 border-green-200",
    pr: "bg-blue-50 text-blue-700 border-blue-200",
    comment: "bg-neutral-50 text-neutral-600 border-neutral-200",
    push: "bg-indigo-50 text-indigo-700 border-indigo-200",
    create: "bg-teal-50 text-teal-700 border-teal-200",
  };

  return (
    <span
      className={cn(
        "rounded-full border px-2 py-0.5 text-xs font-medium",
        colors[type] || "border-neutral-200 bg-neutral-50 text-neutral-600",
      )}
    >
      {type}
    </span>
  );
}

function LeadRow({
  lead,
  isExpanded,
  onToggle,
  onResearch,
  isResearching,
}: {
  lead: StarLead;
  isExpanded: boolean;
  onToggle: () => void;
  onResearch: () => void;
  isResearching: boolean;
}) {
  return (
    <>
      <tr
        className={cn(
          "cursor-pointer border-b border-neutral-100 transition-colors hover:bg-neutral-50",
          isExpanded && "bg-neutral-50",
        )}
        onClick={onToggle}
      >
        <td className="px-3 py-2.5">
          {lead.researched_at ? (
            isExpanded ? (
              <ChevronDownIcon className="h-4 w-4 text-neutral-400" />
            ) : (
              <ChevronRightIcon className="h-4 w-4 text-neutral-400" />
            )
          ) : (
            <div className="h-4 w-4" />
          )}
        </td>
        <td className="px-3 py-2.5">
          <div className="flex items-center gap-2.5">
            {lead.avatar_url ? (
              <img
                src={lead.avatar_url}
                alt={lead.github_username}
                className="h-7 w-7 rounded-full"
              />
            ) : (
              <div className="h-7 w-7 rounded-full bg-neutral-200" />
            )}
            <div>
              <div className="flex items-center gap-1.5">
                <span className="font-medium text-neutral-900">
                  {lead.name || lead.github_username}
                </span>
                <a
                  href={
                    lead.profile_url ||
                    `https://github.com/${lead.github_username}`
                  }
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={(e) => e.stopPropagation()}
                  className="text-neutral-400 hover:text-neutral-600"
                >
                  <ExternalLinkIcon className="h-3 w-3" />
                </a>
              </div>
              {lead.name && (
                <span className="text-xs text-neutral-500">
                  @{lead.github_username}
                </span>
              )}
            </div>
          </div>
        </td>
        <td className="px-3 py-2.5">
          <EventBadge type={lead.event_type} />
        </td>
        <td className="px-3 py-2.5">
          <ScoreBadge score={lead.score} />
        </td>
        <td className="px-3 py-2.5">
          {lead.is_match === null ? (
            <span className="text-neutral-400">--</span>
          ) : lead.is_match ? (
            <span className="font-medium text-green-600">Yes</span>
          ) : (
            <span className="text-neutral-400">No</span>
          )}
        </td>
        <td className="px-3 py-2.5 text-neutral-600">{lead.company || "--"}</td>
        <td className="px-3 py-2.5">
          {lead.researched_at ? (
            <span className="text-xs font-medium text-green-600">
              Researched
            </span>
          ) : (
            <span className="text-xs text-neutral-400">Pending</span>
          )}
        </td>
        <td className="px-3 py-2.5 text-right">
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onResearch();
            }}
            disabled={isResearching}
            className="inline-flex items-center gap-1 rounded-md bg-neutral-900 px-2.5 py-1 text-xs font-medium text-white transition-colors hover:bg-neutral-800 disabled:opacity-50"
          >
            {isResearching ? (
              <Spinner size={12} color="white" />
            ) : (
              <SparklesIcon className="h-3 w-3" />
            )}
            {lead.researched_at ? "Re-research" : "Research"}
          </button>
        </td>
      </tr>
      {isExpanded && lead.reasoning && (
        <tr className="border-b border-neutral-100">
          <td colSpan={8} className="bg-neutral-50 px-6 py-4">
            <div className="max-w-2xl">
              <div className="mb-3 flex items-center gap-4 text-xs text-neutral-500">
                {lead.bio && <span>Bio: {lead.bio}</span>}
                {lead.researched_at && (
                  <span>
                    Researched:{" "}
                    {new Date(lead.researched_at).toLocaleDateString()}
                  </span>
                )}
              </div>
              <div className="prose prose-sm prose-neutral max-w-none">
                <pre className="rounded-lg border border-neutral-200 bg-white p-3 font-sans text-xs whitespace-pre-wrap">
                  {lead.reasoning}
                </pre>
              </div>
            </div>
          </td>
        </tr>
      )}
    </>
  );
}
