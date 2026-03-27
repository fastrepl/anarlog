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
  type UserTemplate,
  type WebTemplate,
} from "./shared";

import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
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
  const deleteTemplateFromStore = main.UI.useDelRowCallback(
    "templates",
    (templateId: string) => templateId,
    main.STORE_ID,
  );

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

  const handleDuplicateTemplate = useCallback(
    (template: UserTemplate) => {
      const id = createTemplate({
        title: getDuplicatedTemplateTitle(template.title),
        description: template.description ?? "",
        sections: template.sections.map((section) => ({ ...section })),
      });

      if (id) {
        setSelectedMineId(id);
      }
    },
    [createTemplate, setSelectedMineId],
  );

  const handleDeleteTemplate = useCallback(
    (id: string) => {
      deleteTemplateFromStore(id);

      if (selectedMineId === id) {
        setSelectedMineId(null);
      }
    },
    [deleteTemplateFromStore, selectedMineId, setSelectedMineId],
  );

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

        <div className="px-2">
          <div
            className={cn([
              "flex h-8 shrink-0 items-center gap-2 rounded-lg border border-neutral-200 bg-neutral-200/50 px-3",
              "transition-colors focus-within:bg-neutral-200",
            ])}
          >
            <Search className="h-4 w-4 shrink-0 text-neutral-400" />
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
              className="min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm placeholder:text-neutral-400 focus:outline-hidden"
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className={cn([
                  "h-4 w-4 shrink-0",
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
            <TemplateListItem
              key={template.id}
              template={template}
              selected={selectedMineId === template.id}
              onSelect={setSelectedMineId}
              onDuplicate={handleDuplicateTemplate}
              onDelete={handleDeleteTemplate}
            />
          ))
        )}
      </div>
    </div>
  );
}

function TemplateListItem({
  template,
  selected,
  onSelect,
  onDuplicate,
  onDelete,
}: {
  template: UserTemplate;
  selected: boolean;
  onSelect: (id: string) => void;
  onDuplicate: (template: UserTemplate) => void;
  onDelete: (id: string) => void;
}) {
  const contextMenu = useMemo(
    () => [
      {
        id: `duplicate-template-${template.id}`,
        text: "Duplicate",
        action: () => onDuplicate(template),
      },
      {
        id: `delete-template-${template.id}`,
        text: "Delete",
        action: () => onDelete(template.id),
      },
    ],
    [onDelete, onDuplicate, template],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <button
      onClick={() => onSelect(template.id)}
      onContextMenu={(e) => {
        onSelect(template.id);
        void showContextMenu(e);
      }}
      className={cn([
        "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
        selected ? "border-neutral-500 bg-neutral-100" : "border-transparent",
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
  );
}

function getDuplicatedTemplateTitle(title: string) {
  const value = title.trim();
  return value ? `${value} copy` : "Untitled copy";
}
