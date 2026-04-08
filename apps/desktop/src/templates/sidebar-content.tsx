import { ArrowDownUp, BookText, Plus, Search, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import {
  resolveTemplateTabSelection,
  useCreateTemplate,
  useToggleTemplateFavorite,
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
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState("");
  const [sortOption, setSortOption] = useState<SortOption>("alphabetical");
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
  const toggleTemplateFavorite = useToggleTemplateFavorite();
  const { data: webTemplates = [], isLoading: isWebLoading } =
    useWebResources<WebTemplate>("templates");
  const deleteTemplateFromStore = main.UI.useDelRowCallback(
    "templates",
    (templateId: string) => templateId,
    main.STORE_ID,
  );

  const {
    isWebMode,
    selectedMineId: effectiveSelectedMineId,
    selectedWebIndex: effectiveSelectedWebIndex,
  } = resolveTemplateTabSelection({
    isWebMode: tab.state.isWebMode,
    selectedMineId: tab.state.selectedMineId,
    selectedWebIndex: tab.state.selectedWebIndex,
    userTemplates,
    webTemplates,
  });

  const setSelectedMineId = useCallback(
    (id: string | null) => {
      updateTabState(tab, {
        ...tab.state,
        isWebMode: false,
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
        category: template.category,
        targets: template.targets,
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

      if (effectiveSelectedMineId === id) {
        setSelectedMineId(null);
      }
    },
    [deleteTemplateFromStore, effectiveSelectedMineId, setSelectedMineId],
  );

  const handleToggleFavorite = useCallback(
    (id: string) => {
      toggleTemplateFavorite(id);
    },
    [toggleTemplateFavorite],
  );

  const sortedUserTemplates = useMemo(() => {
    const favorites = userTemplates
      .filter((template) => template.pinned)
      .sort((a, b) => {
        const orderA = a.pin_order ?? Infinity;
        const orderB = b.pin_order ?? Infinity;
        if (orderA !== orderB) {
          return orderA - orderB;
        }
        return (a.title || "").localeCompare(b.title || "");
      });

    const others = userTemplates.filter((template) => !template.pinned);
    switch (sortOption) {
      case "alphabetical":
        others.sort((a, b) => (a.title || "").localeCompare(b.title || ""));
        break;
      case "reverse-alphabetical":
      default:
        others.sort((a, b) => (b.title || "").localeCompare(a.title || ""));
        break;
    }

    return [...favorites, ...others];
  }, [userTemplates, sortOption]);

  const filteredMine = useMemo(() => {
    if (!search.trim()) return sortedUserTemplates;
    const q = search.toLowerCase();
    return sortedUserTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(q) ||
        template.description?.toLowerCase().includes(q) ||
        template.category?.toLowerCase().includes(q) ||
        template.targets?.some((target) => target.toLowerCase().includes(q)),
    );
  }, [sortedUserTemplates, search]);

  const filteredWeb = useMemo(() => {
    const query = search.toLowerCase().trim();

    const matchingTemplates = webTemplates.flatMap((template, index) => {
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

    matchingTemplates.sort((a, b) => {
      const titleA = a.template.title || "";
      const titleB = b.template.title || "";

      return sortOption === "reverse-alphabetical"
        ? titleB.localeCompare(titleA)
        : titleA.localeCompare(titleB);
    });

    return matchingTemplates;
  }, [search, sortOption, webTemplates]);

  const combinedTemplates = useMemo<
    Array<
      | {
          key: string;
          title: string;
          selected: boolean;
          pinned: boolean;
          source: "user";
          template: UserTemplate;
        }
      | {
          key: string;
          title: string;
          selected: boolean;
          pinned: false;
          source: "web";
          index: number;
          template: WebTemplate;
        }
    >
  >(() => {
    const mine = filteredMine.map((template) => ({
      key: template.id,
      title: template.title?.trim() || "Untitled",
      selected: !isWebMode && effectiveSelectedMineId === template.id,
      pinned: Boolean(template.pinned),
      source: "user" as const,
      template,
    }));

    const web = filteredWeb.map(({ template, index }) => ({
      key: template.slug || `web-${index}`,
      title: template.title?.trim() || "Untitled",
      selected: isWebMode && effectiveSelectedWebIndex === index,
      pinned: false as const,
      source: "web" as const,
      index,
      template,
    }));

    return [...mine, ...web];
  }, [
    effectiveSelectedMineId,
    effectiveSelectedWebIndex,
    filteredMine,
    filteredWeb,
    isWebMode,
  ]);

  const hasResults = combinedTemplates.length > 0;
  const isEmpty = !isWebLoading && !hasResults;

  const selectCombinedTemplate = useCallback(
    (
      item:
        | {
            source: "user";
            template: UserTemplate;
          }
        | {
            source: "web";
            index: number;
          },
    ) => {
      if (item.source === "user") {
        setSelectedMineId(item.template.id);
        return;
      }

      setSelectedWebIndex(item.index);
    },
    [setSelectedMineId, setSelectedWebIndex],
  );

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !event.altKey ||
        event.metaKey ||
        event.ctrlKey ||
        event.shiftKey ||
        (event.key !== "ArrowUp" && event.key !== "ArrowDown")
      ) {
        return;
      }

      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          target.closest("input, textarea, select, [contenteditable='true']"))
      ) {
        return;
      }

      if (combinedTemplates.length === 0) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      const currentIndex = combinedTemplates.findIndex((item) => item.selected);
      const nextIndex =
        currentIndex === -1
          ? event.key === "ArrowDown"
            ? 0
            : combinedTemplates.length - 1
          : Math.max(
              0,
              Math.min(
                combinedTemplates.length - 1,
                currentIndex + (event.key === "ArrowDown" ? 1 : -1),
              ),
            );

      const nextItem = combinedTemplates[nextIndex];
      if (!nextItem || nextIndex === currentIndex) {
        return;
      }

      selectCombinedTemplate(nextItem);
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  }, [combinedTemplates, selectCombinedTemplate]);

  useEffect(() => {
    const selectedElement = scrollContainerRef.current?.querySelector(
      "[data-template-selected='true']",
    );

    if (!(selectedElement instanceof HTMLElement)) {
      return;
    }

    selectedElement.scrollIntoView({
      block: "nearest",
    });
  }, [effectiveSelectedMineId, effectiveSelectedWebIndex]);

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <div>
        <div className="flex h-12 items-center justify-between py-2 pr-1 pl-3">
          <h3 className="font-serif text-sm font-medium">Templates</h3>
          <div className="flex items-center">
            {userTemplates.length > 1 && (
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

        <div className="px-2 pb-2">
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

      <div
        ref={scrollContainerRef}
        className="scrollbar-hide flex-1 overflow-y-auto"
      >
        {isEmpty ? (
          <div className="px-3 py-8 text-center text-neutral-500">
            <BookText size={32} className="mx-auto mb-2 text-neutral-300" />
            <p className="text-sm">
              {search ? "No templates found" : "No templates yet"}
            </p>
            {!search && (
              <button
                onClick={handleCreateTemplate}
                className="mt-3 text-sm text-neutral-600 underline hover:text-neutral-800"
              >
                Create my first template
              </button>
            )}
          </div>
        ) : (
          <>
            {hasResults && (
              <div className="pt-3">
                {combinedTemplates.map((item) =>
                  item.source === "user" ? (
                    <TemplateListItem
                      key={item.key}
                      template={item.template}
                      selected={item.selected}
                      onSelect={setSelectedMineId}
                      onToggleFavorite={handleToggleFavorite}
                      onDuplicate={handleDuplicateTemplate}
                      onDelete={handleDeleteTemplate}
                    />
                  ) : (
                    <button
                      key={item.key}
                      onClick={() => setSelectedWebIndex(item.index)}
                      data-template-selected={item.selected}
                      className={cn([
                        "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
                        item.selected
                          ? "bg-neutral-200"
                          : "hover:bg-neutral-200/50",
                      ])}
                    >
                      <div className="flex items-center gap-2">
                        <BookText className="h-4 w-4 shrink-0 text-neutral-500" />
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-medium">
                            {item.title}
                          </div>
                        </div>
                      </div>
                    </button>
                  ),
                )}
              </div>
            )}

            {isWebLoading && !hasResults && (
              <div className="pt-3">
                <div className="flex flex-col gap-1">
                  {[0, 1, 2, 3].map((index) => (
                    <div
                      key={index}
                      className="animate-pulse rounded-lg px-3 py-2"
                    >
                      <div className="h-4 w-3/4 rounded-xs bg-neutral-200" />
                      <div className="mt-1.5 h-3 w-1/3 rounded-xs bg-neutral-100" />
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function TemplateListItem({
  template,
  selected,
  onSelect,
  onToggleFavorite,
  onDuplicate,
  onDelete,
}: {
  template: UserTemplate;
  selected: boolean;
  onSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
  onDuplicate: (template: UserTemplate) => void;
  onDelete: (id: string) => void;
}) {
  const contextMenu = useMemo(
    () => [
      {
        id: `favorite-template-${template.id}`,
        text: template.pinned ? "Unfavorite" : "Favorite",
        action: () => onToggleFavorite(template.id),
      },
      { separator: true as const },
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
    [onDelete, onDuplicate, onToggleFavorite, template],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <button
      onClick={() => onSelect(template.id)}
      onContextMenu={(e) => {
        onSelect(template.id);
        void showContextMenu(e);
      }}
      data-template-selected={selected}
      className={cn([
        "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
        selected ? "bg-neutral-200" : "hover:bg-neutral-200/50",
      ])}
    >
      <div className="flex items-center gap-2">
        <BookText className="h-4 w-4 shrink-0 text-neutral-500" />
        <div className="min-w-0 flex-1">
          <div className="truncate font-medium">
            {template.title?.trim() || "Untitled"}
          </div>
        </div>
      </div>
    </button>
  );
}

function getDuplicatedTemplateTitle(title: string) {
  const value = title.trim();
  return value ? `${value} copy` : "Untitled copy";
}
