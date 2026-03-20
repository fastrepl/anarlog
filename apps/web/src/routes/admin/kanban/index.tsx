import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  ExternalLinkIcon,
  GripVerticalIcon,
  PlusIcon,
  RefreshCwIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import type {
  ProjectItem,
  ProjectV2,
  StatusOption,
} from "@/functions/github-projects";

export const Route = createFileRoute("/admin/kanban/")({
  component: KanbanPage,
});

function KanbanPage() {
  const queryClient = useQueryClient();
  const [selectedProject, setSelectedProject] = useState<ProjectV2 | null>(
    null,
  );
  const [isCreating, setIsCreating] = useState(false);
  const [editingItem, setEditingItem] = useState<ProjectItem | null>(null);

  const { data: projectsData, isLoading: projectsLoading } = useQuery({
    queryKey: ["kanban-projects"],
    queryFn: async () => {
      const response = await fetch("/api/admin/kanban/projects");
      if (!response.ok) throw new Error("Failed to fetch projects");
      return response.json() as Promise<{ projects: ProjectV2[] }>;
    },
  });

  const projects = projectsData?.projects ?? [];

  const activeProject = selectedProject ?? projects[0] ?? null;

  const {
    data: itemsData,
    isLoading: itemsLoading,
    refetch: refetchItems,
  } = useQuery({
    queryKey: ["kanban-items", activeProject?.number, activeProject?.id],
    queryFn: async () => {
      if (!activeProject)
        return { items: [], statusField: { fieldId: "", options: [] } };
      const params = new URLSearchParams({
        projectNumber: String(activeProject.number),
        projectId: activeProject.id,
      });
      const response = await fetch(`/api/admin/kanban/items?${params}`);
      if (!response.ok) throw new Error("Failed to fetch items");
      return response.json() as Promise<{
        items: ProjectItem[];
        statusField: { fieldId: string; options: StatusOption[] };
      }>;
    },
    enabled: !!activeProject,
  });

  const items = itemsData?.items ?? [];
  const statusField = itemsData?.statusField ?? { fieldId: "", options: [] };

  const columns = useMemo(() => {
    const statusOptions = statusField.options;
    if (statusOptions.length === 0) {
      const statusGroups = new Map<string, ProjectItem[]>();
      for (const item of items) {
        const status = item.status ?? "No Status";
        if (!statusGroups.has(status)) {
          statusGroups.set(status, []);
        }
        statusGroups.get(status)!.push(item);
      }
      return Array.from(statusGroups.entries()).map(([name, columnItems]) => ({
        id: name,
        name,
        items: columnItems,
      }));
    }

    const cols = statusOptions.map((opt) => ({
      id: opt.id,
      name: opt.name,
      items: items.filter((item) => item.status === opt.name),
    }));

    const noStatus = items.filter(
      (item) =>
        !item.status || !statusOptions.some((opt) => opt.name === item.status),
    );
    if (noStatus.length > 0) {
      cols.unshift({ id: "no-status", name: "No Status", items: noStatus });
    }

    return cols;
  }, [items, statusField.options]);

  const createMutation = useMutation({
    mutationFn: async (data: {
      title: string;
      body: string;
      labels?: string[];
    }) => {
      const response = await fetch("/api/admin/kanban/create", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ...data,
          projectId: activeProject?.id,
        }),
      });
      if (!response.ok) throw new Error("Failed to create issue");
      return response.json() as Promise<{
        issue: { id: string; number: number; url: string };
        warning?: string;
      }>;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["kanban-items"] });
      setIsCreating(false);
    },
  });

  const updateMutation = useMutation({
    mutationFn: async (data: {
      issueId?: string;
      title?: string;
      body?: string;
      projectId?: string;
      itemId?: string;
      fieldId?: string;
      optionId?: string;
    }) => {
      const response = await fetch("/api/admin/kanban/update", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
      });
      if (!response.ok) throw new Error("Failed to update");
      return response.json() as Promise<{ success: boolean }>;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["kanban-items"] });
      setEditingItem(null);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (data: {
      issueId: string;
      projectId?: string;
      itemId?: string;
    }) => {
      const response = await fetch("/api/admin/kanban/delete", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
      });
      if (!response.ok) throw new Error("Failed to delete");
      return response.json() as Promise<{ success: boolean }>;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["kanban-items"] });
    },
  });

  const handleStatusChange = useCallback(
    (item: ProjectItem, newStatusOptionId: string) => {
      if (!activeProject) return;
      updateMutation.mutate({
        projectId: activeProject.id,
        itemId: item.id,
        fieldId: statusField.fieldId,
        optionId: newStatusOptionId,
      });
    },
    [activeProject, statusField.fieldId, updateMutation],
  );

  const handleDelete = useCallback(
    (item: ProjectItem) => {
      deleteMutation.mutate({
        issueId: item.issueId,
        projectId: activeProject?.id,
        itemId: item.id,
      });
    },
    [activeProject, deleteMutation],
  );

  if (projectsLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Spinner size={24} />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-200 bg-white px-6 py-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h1 className="text-lg font-semibold text-neutral-900">Kanban</h1>
            <span className="text-sm text-neutral-500">fastrepl/marketing</span>
            {projects.length > 1 && (
              <select
                value={activeProject?.id ?? ""}
                onChange={(e) => {
                  const p = projects.find((pr) => pr.id === e.target.value);
                  if (p) setSelectedProject(p);
                }}
                className="h-8 rounded-lg border border-neutral-200 px-2 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
              >
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.title}
                  </option>
                ))}
              </select>
            )}
            {activeProject && (
              <span className="text-xs text-neutral-400">
                {items.length} items
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => refetchItems()}
              disabled={itemsLoading}
              className="flex h-8 items-center gap-1.5 rounded-lg border border-neutral-200 px-3 text-sm transition-colors hover:bg-neutral-50 disabled:opacity-50"
            >
              <RefreshCwIcon
                className={cn("h-3.5 w-3.5", itemsLoading && "animate-spin")}
              />
              Refresh
            </button>
            <button
              type="button"
              onClick={() => setIsCreating(true)}
              className="flex h-8 items-center gap-1.5 rounded-lg bg-neutral-900 px-3 text-sm text-white transition-colors hover:bg-neutral-800"
            >
              <PlusIcon className="h-3.5 w-3.5" />
              New Issue
            </button>
          </div>
        </div>
      </div>

      {projects.length === 0 ? (
        <div className="flex h-64 flex-col items-center justify-center text-neutral-500">
          <p className="text-sm font-medium">No projects found</p>
          <p className="mt-1 text-xs">
            Create a GitHub Project in fastrepl/marketing to get started
          </p>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-x-auto p-4">
          {itemsLoading ? (
            <div className="flex h-64 items-center justify-center">
              <Spinner size={24} />
            </div>
          ) : (
            <div className="flex h-full min-w-max gap-4">
              {columns.map((column) => (
                <KanbanColumn
                  key={column.id}
                  name={column.name}
                  items={column.items}
                  statusOptions={statusField.options}
                  onStatusChange={handleStatusChange}
                  onEdit={setEditingItem}
                  onDelete={handleDelete}
                  isUpdating={updateMutation.isPending}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {isCreating && (
        <CreateIssueModal
          onClose={() => setIsCreating(false)}
          onSubmit={(title, body) => createMutation.mutate({ title, body })}
          isPending={createMutation.isPending}
        />
      )}

      {editingItem && (
        <EditIssueModal
          item={editingItem}
          onClose={() => setEditingItem(null)}
          onSubmit={(title, body) =>
            updateMutation.mutate({
              issueId: editingItem.issueId,
              title,
              body,
            })
          }
          isPending={updateMutation.isPending}
        />
      )}
    </div>
  );
}

function KanbanColumn({
  name,
  items,
  statusOptions,
  onStatusChange,
  onEdit,
  onDelete,
  isUpdating,
}: {
  name: string;
  items: ProjectItem[];
  statusOptions: StatusOption[];
  onStatusChange: (item: ProjectItem, optionId: string) => void;
  onEdit: (item: ProjectItem) => void;
  onDelete: (item: ProjectItem) => void;
  isUpdating: boolean;
}) {
  const COLUMN_COLORS: Record<string, string> = {
    Todo: "border-t-blue-400",
    "In Progress": "border-t-yellow-400",
    Done: "border-t-green-400",
    Backlog: "border-t-neutral-300",
    "No Status": "border-t-neutral-200",
  };

  return (
    <div
      className={cn(
        "flex w-72 flex-col rounded-lg border border-t-2 border-neutral-200 bg-neutral-50",
        COLUMN_COLORS[name] || "border-t-neutral-300",
      )}
    >
      <div className="flex items-center justify-between px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-neutral-700">{name}</span>
          <span className="rounded-full bg-neutral-200 px-1.5 py-0.5 text-xs text-neutral-400">
            {items.length}
          </span>
        </div>
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-2 pb-2">
        {items.map((item) => (
          <KanbanCard
            key={item.id}
            item={item}
            statusOptions={statusOptions}
            onStatusChange={onStatusChange}
            onEdit={onEdit}
            onDelete={onDelete}
            isUpdating={isUpdating}
          />
        ))}
      </div>
    </div>
  );
}

function KanbanCard({
  item,
  statusOptions,
  onStatusChange,
  onEdit,
  onDelete,
  isUpdating,
}: {
  item: ProjectItem;
  statusOptions: StatusOption[];
  onStatusChange: (item: ProjectItem, optionId: string) => void;
  onEdit: (item: ProjectItem) => void;
  onDelete: (item: ProjectItem) => void;
  isUpdating: boolean;
}) {
  const [showActions, setShowActions] = useState(false);

  return (
    <div
      className="rounded-md border border-neutral-200 bg-white shadow-xs transition-shadow hover:shadow-sm"
      onMouseEnter={() => setShowActions(true)}
      onMouseLeave={() => setShowActions(false)}
    >
      <div className="p-3">
        <div className="flex items-start gap-1.5">
          <GripVerticalIcon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-neutral-300" />
          <div className="min-w-0 flex-1">
            <div className="flex items-start justify-between gap-1">
              <button
                type="button"
                onClick={() => onEdit(item)}
                className="truncate text-left text-sm font-medium text-neutral-900 hover:text-blue-600"
              >
                {item.title}
              </button>
              {showActions && (
                <div className="flex shrink-0 items-center gap-0.5">
                  <a
                    href={item.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="rounded p-0.5 text-neutral-400 hover:text-neutral-600"
                  >
                    <ExternalLinkIcon className="h-3 w-3" />
                  </a>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete(item);
                    }}
                    className="rounded p-0.5 text-neutral-400 hover:text-red-500"
                  >
                    <Trash2Icon className="h-3 w-3" />
                  </button>
                </div>
              )}
            </div>
            <span className="text-xs text-neutral-400">
              #{item.issueNumber}
            </span>
          </div>
        </div>

        {item.labels.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1">
            {item.labels.map((label) => (
              <span
                key={label}
                className="rounded bg-neutral-100 px-1.5 py-0.5 text-[10px] font-medium text-neutral-600"
              >
                {label}
              </span>
            ))}
          </div>
        )}

        {statusOptions.length > 0 && (
          <div className="mt-2">
            <select
              value={
                statusOptions.find((o) => o.name === item.status)?.id ?? ""
              }
              onChange={(e) => onStatusChange(item, e.target.value)}
              disabled={isUpdating}
              className="h-6 w-full rounded border border-neutral-200 px-1 text-[11px] focus:ring-1 focus:ring-neutral-300 focus:outline-none disabled:opacity-50"
            >
              <option value="">No Status</option>
              {statusOptions.map((opt) => (
                <option key={opt.id} value={opt.id}>
                  {opt.name}
                </option>
              ))}
            </select>
          </div>
        )}

        {item.assignees.length > 0 && (
          <div className="mt-2 flex items-center gap-1">
            {item.assignees.map((assignee) => (
              <span
                key={assignee}
                className="rounded bg-neutral-50 px-1.5 py-0.5 text-[10px] text-neutral-500"
              >
                @{assignee}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function CreateIssueModal({
  onClose,
  onSubmit,
  isPending,
}: {
  onClose: () => void;
  onSubmit: (title: string, body: string) => void;
  isPending: boolean;
}) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    onSubmit(title.trim(), body.trim());
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="mx-4 w-full max-w-lg rounded-xl bg-white shadow-xl">
        <div className="flex items-center justify-between border-b border-neutral-200 px-5 py-3">
          <h2 className="text-sm font-semibold text-neutral-900">
            Create Issue
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-neutral-400 hover:text-neutral-600"
          >
            <XIcon className="h-4 w-4" />
          </button>
        </div>
        <form onSubmit={handleSubmit} className="space-y-3 p-5">
          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500">
              Title
            </label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Issue title..."
              className="h-9 w-full rounded-lg border border-neutral-200 px-3 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
              autoFocus
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500">
              Description
            </label>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder="Describe the issue..."
              rows={5}
              className="w-full resize-none rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
            />
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="h-8 rounded-lg px-4 text-sm text-neutral-600 transition-colors hover:bg-neutral-100"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!title.trim() || isPending}
              className="flex h-8 items-center gap-1.5 rounded-lg bg-neutral-900 px-4 text-sm text-white transition-colors hover:bg-neutral-800 disabled:opacity-50"
            >
              {isPending && <Spinner size={12} color="white" />}
              Create
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function EditIssueModal({
  item,
  onClose,
  onSubmit,
  isPending,
}: {
  item: ProjectItem;
  onClose: () => void;
  onSubmit: (title: string, body: string) => void;
  isPending: boolean;
}) {
  const [title, setTitle] = useState(item.title);
  const [body, setBody] = useState(item.body);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    onSubmit(title.trim(), body.trim());
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="mx-4 w-full max-w-lg rounded-xl bg-white shadow-xl">
        <div className="flex items-center justify-between border-b border-neutral-200 px-5 py-3">
          <h2 className="text-sm font-semibold text-neutral-900">
            Edit Issue #{item.issueNumber}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-neutral-400 hover:text-neutral-600"
          >
            <XIcon className="h-4 w-4" />
          </button>
        </div>
        <form onSubmit={handleSubmit} className="space-y-3 p-5">
          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500">
              Title
            </label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="h-9 w-full rounded-lg border border-neutral-200 px-3 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500">
              Description
            </label>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              rows={8}
              className="w-full resize-none rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:ring-2 focus:ring-neutral-300 focus:outline-none"
            />
          </div>
          <div className="flex items-center justify-between pt-1">
            <a
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1 text-xs text-blue-600 hover:underline"
            >
              <ExternalLinkIcon className="h-3 w-3" />
              View on GitHub
            </a>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={onClose}
                className="h-8 rounded-lg px-4 text-sm text-neutral-600 transition-colors hover:bg-neutral-100"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={!title.trim() || isPending}
                className="flex h-8 items-center gap-1.5 rounded-lg bg-neutral-900 px-4 text-sm text-white transition-colors hover:bg-neutral-800 disabled:opacity-50"
              >
                {isPending && <Spinner size={12} color="white" />}
                Save
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
