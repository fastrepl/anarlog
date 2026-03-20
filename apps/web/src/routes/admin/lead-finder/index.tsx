import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  ExternalLinkIcon,
  RefreshCwIcon,
  SearchIcon,
  SparklesIcon,
  StarIcon,
  XIcon,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import type { StarLead } from "@/functions/github-stars";

export const Route = createFileRoute("/admin/lead-finder/")({
  component: LeadFinderPage,
});

function LeadFinderPage() {
  const queryClient = useQueryClient();
  const [page, setPage] = useState(0);
  const [searchQuery, setSearchQuery] = useState("");
  const [showResearchedOnly, setShowResearchedOnly] = useState(false);
  const [selectedLead, setSelectedLead] = useState<StarLead | null>(null);
  const pageSize = 50;

  const { data, isLoading, isFetching } = useQuery({
    queryKey: ["star-leads", page, showResearchedOnly],
    queryFn: async () => {
      const params = new URLSearchParams({
        limit: String(pageSize),
        offset: String(page * pageSize),
        ...(showResearchedOnly ? { researched: "true" } : {}),
      });
      const response = await fetch(`/api/admin/stars/leads?${params}`);
      if (!response.ok) throw new Error("Failed to fetch leads");
      return response.json() as Promise<{ leads: StarLead[]; total: number }>;
    },
    staleTime: 30000,
  });

  const fetchMutation = useMutation({
    mutationFn: async (source: "stargazers" | "activity") => {
      const response = await fetch("/api/admin/stars/fetch", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ source }),
      });
      if (!response.ok) throw new Error("Failed to fetch");
      return response.json() as Promise<{ added: number; total: number }>;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["star-leads"] });
    },
  });

  const researchMutation = useMutation({
    mutationFn: async (username: string) => {
      const response = await fetch("/api/admin/stars/research", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username }),
      });
      if (!response.ok) throw new Error("Failed to research");
      return response.json() as Promise<{
        success: boolean;
        lead?: StarLead;
        error?: string;
      }>;
    },
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["star-leads"] });
      if (data.lead) {
        setSelectedLead(data.lead);
      }
    },
  });

  const leads = data?.leads ?? [];
  const total = data?.total ?? 0;
  const totalPages = Math.ceil(total / pageSize);

  const filteredLeads = useMemo(() => {
    if (!searchQuery) return leads;
    const q = searchQuery.toLowerCase();
    return leads.filter(
      (l) =>
        l.github_username.toLowerCase().includes(q) ||
        (l.name && l.name.toLowerCase().includes(q)) ||
        (l.company && l.company.toLowerCase().includes(q)),
    );
  }, [leads, searchQuery]);

  const handleResearchAll = useCallback(async () => {
    const unresearched = leads.filter((l) => !l.researched_at);
    for (const lead of unresearched.slice(0, 10)) {
      await researchMutation.mutateAsync(lead.github_username);
    }
  }, [leads, researchMutation]);

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-200 bg-white px-6 py-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="relative">
              <SearchIcon className="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-neutral-400" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search leads..."
                className="h-8 w-64 rounded-lg border border-neutral-200 pr-3 pl-9 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => setSearchQuery("")}
                  className="absolute top-1/2 right-2 -translate-y-1/2 text-neutral-400 hover:text-neutral-600"
                >
                  <XIcon className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
            <label className="flex cursor-pointer items-center gap-1.5 text-sm text-neutral-600">
              <input
                type="checkbox"
                checked={showResearchedOnly}
                onChange={(e) => {
                  setShowResearchedOnly(e.target.checked);
                  setPage(0);
                }}
                className="rounded border-neutral-300"
              />
              Researched only
            </label>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-neutral-500">
              {total} total leads
            </span>
            <button
              type="button"
              onClick={() => fetchMutation.mutate("stargazers")}
              disabled={fetchMutation.isPending}
              className="flex h-8 items-center gap-1.5 rounded-lg border border-neutral-200 px-3 text-sm transition-colors hover:bg-neutral-50 disabled:opacity-50"
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
              className="flex h-8 items-center gap-1.5 rounded-lg border border-neutral-200 px-3 text-sm transition-colors hover:bg-neutral-50 disabled:opacity-50"
            >
              {fetchMutation.isPending ? (
                <Spinner size={14} />
              ) : (
                <RefreshCwIcon className="h-3.5 w-3.5" />
              )}
              Fetch Activity
            </button>
            <button
              type="button"
              onClick={handleResearchAll}
              disabled={researchMutation.isPending}
              className="flex h-8 items-center gap-1.5 rounded-lg bg-neutral-900 px-3 text-sm text-white transition-colors hover:bg-neutral-800 disabled:opacity-50"
            >
              {researchMutation.isPending ? (
                <Spinner size={14} color="white" />
              ) : (
                <SparklesIcon className="h-3.5 w-3.5" />
              )}
              Research Top 10
            </button>
          </div>
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        <div className="min-w-0 flex-1 overflow-auto">
          {isLoading ? (
            <div className="flex h-64 items-center justify-center">
              <Spinner size={24} />
            </div>
          ) : (
            <table className="w-full text-sm">
              <thead className="sticky top-0 border-b border-neutral-200 bg-neutral-50">
                <tr>
                  <th className="px-4 py-2 text-left font-medium text-neutral-600">
                    User
                  </th>
                  <th className="px-4 py-2 text-left font-medium text-neutral-600">
                    Event
                  </th>
                  <th className="px-4 py-2 text-left font-medium text-neutral-600">
                    Company
                  </th>
                  <th className="px-4 py-2 text-center font-medium text-neutral-600">
                    Score
                  </th>
                  <th className="px-4 py-2 text-center font-medium text-neutral-600">
                    Match
                  </th>
                  <th className="px-4 py-2 text-right font-medium text-neutral-600">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody>
                {filteredLeads.map((lead) => (
                  <LeadRow
                    key={lead.id}
                    lead={lead}
                    isSelected={selectedLead?.id === lead.id}
                    onSelect={() => setSelectedLead(lead)}
                    onResearch={() =>
                      researchMutation.mutate(lead.github_username)
                    }
                    isResearching={
                      researchMutation.isPending &&
                      researchMutation.variables === lead.github_username
                    }
                  />
                ))}
                {filteredLeads.length === 0 && (
                  <tr>
                    <td
                      colSpan={6}
                      className="py-12 text-center text-neutral-500"
                    >
                      {searchQuery
                        ? "No leads match your search"
                        : "No leads found. Click 'Fetch Stars' to import stargazers."}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </div>

        {selectedLead && (
          <div className="w-96 overflow-auto border-l border-neutral-200 bg-white">
            <LeadDetail
              lead={selectedLead}
              onClose={() => setSelectedLead(null)}
              onResearch={() =>
                researchMutation.mutate(selectedLead.github_username)
              }
              isResearching={researchMutation.isPending}
            />
          </div>
        )}
      </div>

      {totalPages > 1 && (
        <div className="flex items-center justify-between border-t border-neutral-200 bg-white px-6 py-2">
          <span className="text-xs text-neutral-500">
            Page {page + 1} of {totalPages}
          </span>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => setPage((p) => Math.max(0, p - 1))}
              disabled={page === 0 || isFetching}
              className="flex h-7 w-7 items-center justify-center rounded hover:bg-neutral-100 disabled:opacity-30"
            >
              <ChevronLeftIcon className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={page >= totalPages - 1 || isFetching}
              className="flex h-7 w-7 items-center justify-center rounded hover:bg-neutral-100 disabled:opacity-30"
            >
              <ChevronRightIcon className="h-4 w-4" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function LeadRow({
  lead,
  isSelected,
  onSelect,
  onResearch,
  isResearching,
}: {
  lead: StarLead;
  isSelected: boolean;
  onSelect: () => void;
  onResearch: () => void;
  isResearching: boolean;
}) {
  return (
    <tr
      onClick={onSelect}
      className={cn(
        "cursor-pointer border-b border-neutral-100 transition-colors",
        isSelected ? "bg-blue-50" : "hover:bg-neutral-50",
      )}
    >
      <td className="px-4 py-2">
        <div className="flex items-center gap-2.5">
          {lead.avatar_url && (
            <img
              src={lead.avatar_url}
              alt={lead.github_username}
              className="h-7 w-7 rounded-full"
            />
          )}
          <div className="min-w-0">
            <div className="truncate font-medium text-neutral-900">
              {lead.name || lead.github_username}
            </div>
            {lead.name && (
              <div className="truncate text-xs text-neutral-500">
                @{lead.github_username}
              </div>
            )}
          </div>
        </div>
      </td>
      <td className="px-4 py-2">
        <span
          className={cn(
            "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium",
            lead.event_type === "star"
              ? "bg-yellow-50 text-yellow-700"
              : lead.event_type === "fork"
                ? "bg-blue-50 text-blue-700"
                : "bg-neutral-100 text-neutral-600",
          )}
        >
          {lead.event_type}
        </span>
      </td>
      <td className="max-w-[150px] truncate px-4 py-2 text-neutral-600">
        {lead.company || "-"}
      </td>
      <td className="px-4 py-2 text-center">
        {lead.score !== null ? (
          <span
            className={cn(
              "inline-flex h-6 w-8 items-center justify-center rounded text-xs font-medium",
              lead.score >= 70
                ? "bg-green-50 text-green-700"
                : lead.score >= 40
                  ? "bg-yellow-50 text-yellow-700"
                  : "bg-neutral-100 text-neutral-500",
            )}
          >
            {lead.score}
          </span>
        ) : (
          <span className="text-neutral-300">-</span>
        )}
      </td>
      <td className="px-4 py-2 text-center">
        {lead.is_match === true ? (
          <span className="text-xs font-medium text-green-600">Yes</span>
        ) : lead.is_match === false ? (
          <span className="text-xs text-neutral-400">No</span>
        ) : (
          <span className="text-neutral-300">-</span>
        )}
      </td>
      <td className="px-4 py-2 text-right">
        <div className="flex items-center justify-end gap-1">
          {!lead.researched_at && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onResearch();
              }}
              disabled={isResearching}
              className="flex h-6 items-center gap-1 rounded border border-neutral-200 px-2 text-xs hover:bg-neutral-50 disabled:opacity-50"
            >
              {isResearching ? (
                <Spinner size={10} />
              ) : (
                <SparklesIcon className="h-3 w-3" />
              )}
              Research
            </button>
          )}
          <a
            href={
              lead.profile_url || `https://github.com/${lead.github_username}`
            }
            target="_blank"
            rel="noopener noreferrer"
            onClick={(e) => e.stopPropagation()}
            className="flex h-6 w-6 items-center justify-center rounded text-neutral-400 hover:bg-neutral-100 hover:text-neutral-600"
          >
            <ExternalLinkIcon className="h-3.5 w-3.5" />
          </a>
        </div>
      </td>
    </tr>
  );
}

function LeadDetail({
  lead,
  onClose,
  onResearch,
  isResearching,
}: {
  lead: StarLead;
  onClose: () => void;
  onResearch: () => void;
  isResearching: boolean;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-3">
        <h3 className="text-sm font-medium text-neutral-900">Lead Details</h3>
        <button
          type="button"
          onClick={onClose}
          className="text-neutral-400 hover:text-neutral-600"
        >
          <XIcon className="h-4 w-4" />
        </button>
      </div>

      <div className="flex-1 space-y-4 overflow-auto p-4">
        <div className="flex items-center gap-3">
          {lead.avatar_url && (
            <img
              src={lead.avatar_url}
              alt={lead.github_username}
              className="h-12 w-12 rounded-full"
            />
          )}
          <div>
            <div className="font-medium text-neutral-900">
              {lead.name || lead.github_username}
            </div>
            <a
              href={
                lead.profile_url || `https://github.com/${lead.github_username}`
              }
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-blue-600 hover:underline"
            >
              @{lead.github_username}
            </a>
          </div>
        </div>

        {lead.bio && (
          <div>
            <div className="mb-1 text-xs font-medium text-neutral-500">Bio</div>
            <p className="text-sm text-neutral-700">{lead.bio}</p>
          </div>
        )}

        <div className="grid grid-cols-2 gap-3">
          <div>
            <div className="mb-1 text-xs font-medium text-neutral-500">
              Event
            </div>
            <span
              className={cn(
                "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium",
                lead.event_type === "star"
                  ? "bg-yellow-50 text-yellow-700"
                  : "bg-neutral-100 text-neutral-600",
              )}
            >
              {lead.event_type}
            </span>
          </div>
          <div>
            <div className="mb-1 text-xs font-medium text-neutral-500">
              Repo
            </div>
            <span className="text-sm text-neutral-700">{lead.repo_name}</span>
          </div>
          {lead.company && (
            <div>
              <div className="mb-1 text-xs font-medium text-neutral-500">
                Company
              </div>
              <span className="text-sm text-neutral-700">{lead.company}</span>
            </div>
          )}
          {lead.score !== null && (
            <div>
              <div className="mb-1 text-xs font-medium text-neutral-500">
                Score
              </div>
              <span
                className={cn(
                  "inline-flex items-center rounded px-2 py-0.5 text-xs font-bold",
                  lead.score >= 70
                    ? "bg-green-50 text-green-700"
                    : lead.score >= 40
                      ? "bg-yellow-50 text-yellow-700"
                      : "bg-neutral-100 text-neutral-500",
                )}
              >
                {lead.score}/100
              </span>
            </div>
          )}
        </div>

        <div className="text-xs text-neutral-400">
          Event at: {new Date(lead.event_at).toLocaleDateString()}
          {lead.researched_at && (
            <>
              {" | "}Researched:{" "}
              {new Date(lead.researched_at).toLocaleDateString()}
            </>
          )}
        </div>

        {lead.reasoning && (
          <div>
            <div className="mb-1 text-xs font-medium text-neutral-500">
              Research Notes
            </div>
            <div className="rounded-lg border border-neutral-100 bg-neutral-50 p-3 text-sm whitespace-pre-wrap text-neutral-700">
              {lead.reasoning}
            </div>
          </div>
        )}

        {!lead.researched_at && (
          <button
            type="button"
            onClick={onResearch}
            disabled={isResearching}
            className="flex h-9 w-full items-center justify-center gap-1.5 rounded-lg bg-neutral-900 text-sm text-white transition-colors hover:bg-neutral-800 disabled:opacity-50"
          >
            {isResearching ? (
              <Spinner size={14} color="white" />
            ) : (
              <SparklesIcon className="h-3.5 w-3.5" />
            )}
            {isResearching ? "Researching..." : "Research This Lead"}
          </button>
        )}
      </div>
    </div>
  );
}
