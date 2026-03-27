import {
  ArrowDownUp,
  BookText,
  Globe,
  Plus,
  Search,
  Star,
  X,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { Switch } from "@hypr/ui/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import {
  useCreateTemplate,
  useUserTemplates,
  type WebTemplate,
} from "./shared";

import { useWebResources } from "~/shared/ui/resource-list";
import { type Tab, useTabs } from "~/store/zustand/tabs";

type SortOption = "alphabetical" | "reverse-alphabetical";

export function TemplatesSidebarContent({
  tab,
}: {
  tab: Extract<Tab, { type: "templates" }>;
}) {
  const updateTabState = useTabs((state) => state.updateTemplatesTabState);
  const [search, setSearch] = useState("");
  const [sortOption, setSortOption] = useState<SortOption>("alphabetical");
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
  const { data: webTemplates = [], isLoading: isWebLoading } =
    useWebResources<WebTemplate>("templates");

  const { selectedMineId, selectedWebIndex } = tab.state;
  const isWebMode = tab.state.isWebMode ?? userTemplates.length === 0;

  const setShowHomepage = useCallback(() => {
    updateTabState(tab, {
      ...tab.state,
      showHomepage: true,
    });
  }, [updateTabState, tab]);

  const setIsWebMode = useCallback(
    (value: boolean) => {
      updateTabState(tab, {
        ...tab.state,
        isWebMode: value,
        selectedMineId: null,
        selectedWebIndex: null,
      });
    },
    [updateTabState, tab],
  );

  const setSelectedMineId = useCallback(
    (id: string | null) => {
      updateTabState(tab, {
        ...tab.state,
        isWebMode: false,
        showHomepage: false,
        selectedMineId: id,
        selectedWebIndex: null,
      });
    },
    [updateTabState, tab],
  );

  const setSelectedWebIndex = useCallback(
    (index: number | null) => {
      updateTabState(tab, {
        ...tab.state,
        isWebMode: true,
        showHomepage: false,
        selectedMineId: null,
        selectedWebIndex: index,
      });
    },
    [updateTabState, tab],
  );

  const handleCreateTemplate = useCallback(() => {
    const id = createTemplate({
      title: "New Template",
      description: "",
      sections: [],
    });

    if (id) {
      setSelectedMineId(id);
    }
  }, [createTemplate, setSelectedMineId]);

  const sortedUserTemplates = useMemo(() => {
    const sorted = [...userTemplates];
    switch (sortOption) {
      case "alphabetical":
        return sorted.sort((a, b) =>
          (a.title || "").localeCompare(b.title || ""),
        );
      case "reverse-alphabetical":
      default:
        return sorted.sort((a, b) =>
          (b.title || "").localeCompare(a.title || ""),
        );
    }
  }, [userTemplates, sortOption]);

  const filteredMine = useMemo(() => {
    if (!search.trim()) return sortedUserTemplates;
    const q = search.toLowerCase();
    return sortedUserTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(q) ||
        template.description?.toLowerCase().includes(q),
    );
  }, [sortedUserTemplates, search]);

  const filteredWeb = useMemo(() => {
    const query = search.toLowerCase().trim();

    return webTemplates.flatMap((template, index) => {
      const matches =
        !query ||
        template.title?.toLowerCase().includes(query) ||
        template.description?.toLowerCase().includes(query) ||
        template.category?.toLowerCase().includes(query) ||
        template.targets?.some((target) =>
          target.toLowerCase().includes(query),
        );

      return matches ? [{ template, index }] : [];
    });
  }, [webTemplates, search]);

  const isEmpty = isWebMode
    ? filteredWeb.length === 0
    : filteredMine.length === 0;

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <div>
        <div className="flex h-12 items-center justify-between py-2 pr-1 pl-3">
          <button
            onClick={setShowHomepage}
            className="text-sm font-medium hover:text-neutral-600"
          >
            Templates
          </button>
          <div className="flex items-center gap-1">
            {!isWebMode && (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="text-neutral-600 hover:text-black"
                  >
                    <ArrowDownUp size={16} />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent variant="app" align="end">
                  <AppFloatingPanel className="overflow-hidden p-1">
                    <DropdownMenuItem
                      onClick={() => setSortOption("alphabetical")}
                    >
                      A to Z
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={() => setSortOption("reverse-alphabetical")}
                    >
                      Z to A
                    </DropdownMenuItem>
                  </AppFloatingPanel>
                </DropdownMenuContent>
              </DropdownMenu>
            )}

            <Tooltip>
              <TooltipTrigger asChild>
                <div className="flex items-center gap-1.5 px-2">
                  <Globe size={14} className="text-neutral-400" />
                  <Switch
                    size="sm"
                    checked={isWebMode}
                    onCheckedChange={setIsWebMode}
                  />
                </div>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                {isWebMode
                  ? "Showing community templates"
                  : "Showing your templates"}
              </TooltipContent>
            </Tooltip>

            <Button
              size="icon"
              variant="ghost"
              className="text-neutral-600 hover:text-black"
              onClick={handleCreateTemplate}
            >
              <Plus size={16} />
            </Button>
          </div>
        </div>

        <div>
          <div className="relative flex h-8 shrink-0 items-center">
            <Search className="absolute left-5 h-4 w-4 text-neutral-400" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearch("");
                }
              }}
              placeholder="Search templates..."
              className={cn([
                "text-sm placeholder:text-sm placeholder:text-neutral-400",
                "h-full w-full pl-8",
                search ? "pr-8" : "pr-4",
                "rounded-lg border border-neutral-200 bg-neutral-200/50",
                "focus:bg-neutral-200 focus:outline-hidden",
              ])}
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className={cn([
                  "absolute right-5",
                  "h-4 w-4",
                  "text-neutral-400 hover:text-neutral-600",
                  "transition-colors",
                ])}
                aria-label="Clear search"
              >
                <X className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-2">
        {isWebMode && isWebLoading ? (
          <div className="flex flex-col gap-2">
            {[0, 1, 2, 3].map((index) => (
              <div key={index} className="animate-pulse rounded-md px-3 py-2">
                <div className="h-4 w-3/4 rounded-xs bg-neutral-200" />
                <div className="mt-1.5 h-3 w-1/2 rounded-xs bg-neutral-100" />
              </div>
            ))}
          </div>
        ) : isEmpty ? (
          <div className="py-8 text-center text-neutral-500">
            {isWebMode ? (
              <BookText size={32} className="mx-auto mb-2 text-neutral-300" />
            ) : (
              <Star size={32} className="mx-auto mb-2 text-neutral-300" />
            )}
            <p className="text-sm">
              {search
                ? "No templates found"
                : isWebMode
                  ? "No community templates"
                  : "No templates yet"}
            </p>
            {!search && !isWebMode && (
              <button
                onClick={handleCreateTemplate}
                className="mt-3 text-sm text-neutral-600 underline hover:text-neutral-800"
              >
                Create your first template
              </button>
            )}
          </div>
        ) : isWebMode ? (
          filteredWeb.map(({ template, index }) => (
            <button
              key={template.slug || index}
              onClick={() => setSelectedWebIndex(index)}
              className={cn([
                "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
                selectedWebIndex === index
                  ? "border-neutral-500 bg-neutral-100"
                  : "border-transparent",
              ])}
            >
              <div className="flex items-center gap-2">
                <BookText className="h-4 w-4 shrink-0 text-neutral-500" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">
                    {template.title || "Untitled"}
                    {template.category && (
                      <span className="ml-1 font-mono text-xs text-stone-400">
                        ({template.category})
                      </span>
                    )}
                  </div>
                  {template.description && (
                    <div className="truncate text-xs text-neutral-500">
                      {template.description}
                    </div>
                  )}
                </div>
              </div>
            </button>
          ))
        ) : (
          filteredMine.map((template) => (
            <button
              key={template.id}
              onClick={() => setSelectedMineId(template.id)}
              className={cn([
                "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
                selectedMineId === template.id
                  ? "border-neutral-500 bg-neutral-100"
                  : "border-transparent",
              ])}
            >
              <div className="flex items-center gap-2">
                <BookText className="h-4 w-4 shrink-0 text-neutral-500" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">
                    {template.title?.trim() || "Untitled"}
                  </div>
                  {template.description && (
                    <div className="truncate text-xs text-neutral-500">
                      {template.description}
                    </div>
                  )}
                </div>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
