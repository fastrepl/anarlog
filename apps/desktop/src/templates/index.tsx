import {
  ArrowDownUp,
  BookText,
  Globe,
  Plus,
  Search,
  Star,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { Template, TemplateSection, TemplateStorage } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { Switch } from "@hypr/ui/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { TemplateDetailsColumn } from "./components/details";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export const TabItemTemplate: TabItem<Extract<Tab, { type: "templates" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<BookTextIcon className="h-4 w-4" />}
      title={"Templates"}
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

function BookTextIcon({ className }: { className?: string }) {
  return <BookText className={className} />;
}

export function TabContentTemplate({
  tab,
}: {
  tab: Extract<Tab, { type: "templates" }>;
}) {
  return (
    <StandardTabWrapper>
      <TemplateView tab={tab} />
    </StandardTabWrapper>
  );
}

type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  sections: TemplateSection[];
};

export type UserTemplate = Template & { id: string };

export function useUserTemplates(): UserTemplate[] {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const queries = main.UI.useQueries(main.STORE_ID);

  useEffect(() => {
    queries?.setParamValue(
      main.QUERIES.userTemplates,
      "user_id",
      user_id ?? "",
    );
  }, [queries, user_id]);

  const templates = main.UI.useResultTable(
    main.QUERIES.userTemplates,
    main.STORE_ID,
  );

  return useMemo(() => {
    return Object.entries(templates).map(([id, template]) =>
      normalizeTemplateWithId(id, template as unknown),
    );
  }, [templates]);
}

function normalizeTemplatePayload(template: unknown): Template {
  const record = (
    template && typeof template === "object" ? template : {}
  ) as Record<string, unknown>;

  let sections: Array<{ title: string; description: string }> = [];
  if (typeof record.sections === "string") {
    try {
      sections = JSON.parse(record.sections);
    } catch {
      sections = [];
    }
  } else if (Array.isArray(record.sections)) {
    sections = record.sections;
  }

  return {
    user_id: typeof record.user_id === "string" ? record.user_id : "",
    title: typeof record.title === "string" ? record.title : "",
    description:
      typeof record.description === "string" ? record.description : "",
    sections,
  };
}

function normalizeTemplateWithId(id: string, template: unknown) {
  return { id, ...normalizeTemplatePayload(template) };
}

function TemplateView({ tab }: { tab: Extract<Tab, { type: "templates" }> }) {
  const updateTabState = useTabs((state) => state.updateTemplatesTabState);
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const leftPanelRef = useRef<ImperativePanelHandle>(null);
  const [isLeftPanelCollapsed, setIsLeftPanelCollapsed] = useState(false);

  const userTemplates = useUserTemplates();
  const { data: webTemplates = [], isLoading: isWebLoading } =
    useWebResources<WebTemplate>("templates");

  const { selectedMineId, selectedWebIndex } = tab.state;
  const showHomepage = tab.state.showHomepage ?? true;
  const isWebMode = tab.state.isWebMode ?? userTemplates.length === 0;

  const setShowHomepage = useCallback(
    (value: boolean) => {
      updateTabState(tab, {
        ...tab.state,
        showHomepage: value,
      });
    },
    [updateTabState, tab],
  );

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

  const selectedWebTemplate =
    selectedWebIndex !== null ? (webTemplates[selectedWebIndex] ?? null) : null;

  const deleteTemplateFromStore = main.UI.useDelRowCallback(
    "templates",
    (template_id: string) => template_id,
    main.STORE_ID,
  );

  const handleDeleteTemplate = useCallback(
    (id: string) => {
      deleteTemplateFromStore(id);
      setSelectedMineId(null);
    },
    [deleteTemplateFromStore, setSelectedMineId],
  );

  const setRow = main.UI.useSetRowCallback(
    "templates",
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      description: string;
      sections: TemplateSection[];
    }) => p.id,
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      description: string;
      sections: TemplateSection[];
    }) =>
      ({
        user_id: p.user_id,
        title: p.title,
        description: p.description,
        sections: JSON.stringify(p.sections),
      }) satisfies TemplateStorage,
    [],
    main.STORE_ID,
  );

  const handleCloneTemplate = useCallback(
    (template: {
      title: string;
      description: string;
      sections: TemplateSection[];
    }) => {
      if (!user_id) return;

      const newId = crypto.randomUUID();
      const now = new Date().toISOString();

      setRow({
        id: newId,
        user_id,
        created_at: now,
        title: template.title,
        description: template.description,
        sections: template.sections.map((section) => ({ ...section })),
      });

      setSelectedMineId(newId);
    },
    [user_id, setRow, setIsWebMode, setSelectedMineId],
  );

  const handleCreateTemplate = useCallback(() => {
    if (!user_id) return;

    const newId = crypto.randomUUID();
    const now = new Date().toISOString();

    setRow({
      id: newId,
      user_id,
      created_at: now,
      title: "New Template",
      description: "",
      sections: [],
    });

    setSelectedMineId(newId);
  }, [user_id, setRow, setIsWebMode, setSelectedMineId]);

  const handleExpandPanel = useCallback(() => {
    leftPanelRef.current?.expand();
  }, []);

  return (
    <ResizablePanelGroup direction="horizontal" className="h-full">
      <ResizablePanel
        ref={leftPanelRef}
        defaultSize={25}
        minSize={15}
        maxSize={35}
        collapsible={showHomepage}
        collapsedSize={0}
        onCollapse={() => setIsLeftPanelCollapsed(true)}
        onExpand={() => setIsLeftPanelCollapsed(false)}
      >
        <TemplateListColumn
          showHomepage={showHomepage}
          isWebMode={isWebMode}
          setIsWebMode={setIsWebMode}
          userTemplates={userTemplates}
          webTemplates={webTemplates}
          isWebLoading={isWebLoading}
          selectedMineId={selectedMineId}
          selectedWebIndex={selectedWebIndex}
          setSelectedMineId={setSelectedMineId}
          setSelectedWebIndex={setSelectedWebIndex}
          setShowHomepage={setShowHomepage}
          onCreateTemplate={handleCreateTemplate}
        />
      </ResizablePanel>
      <ResizableHandle className={isLeftPanelCollapsed ? "w-0" : undefined} />
      <ResizablePanel defaultSize={75}>
        {showHomepage ? (
          <TemplatesHomepage
            webTemplates={webTemplates}
            isWebLoading={isWebLoading}
            onSelectWebTemplate={setSelectedWebIndex}
            onCreateTemplate={handleCreateTemplate}
            isSidebarCollapsed={isLeftPanelCollapsed}
            onExpandSidebar={handleExpandPanel}
          />
        ) : (
          <TemplateDetailsColumn
            isWebMode={isWebMode}
            selectedMineId={selectedMineId}
            selectedWebTemplate={selectedWebTemplate}
            handleDeleteTemplate={handleDeleteTemplate}
            handleCloneTemplate={handleCloneTemplate}
          />
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}

type SortOption = "alphabetical" | "reverse-alphabetical";

function TemplatesHomepage({
  webTemplates,
  isWebLoading,
  onSelectWebTemplate,
  onCreateTemplate,
  isSidebarCollapsed,
  onExpandSidebar,
}: {
  webTemplates: WebTemplate[];
  isWebLoading: boolean;
  onSelectWebTemplate: (index: number) => void;
  onCreateTemplate: () => void;
  isSidebarCollapsed: boolean;
  onExpandSidebar: () => void;
}) {
  const [search, setSearch] = useState("");

  const filteredTemplates = useMemo(() => {
    if (!search.trim()) return webTemplates;
    const q = search.toLowerCase();
    return webTemplates.filter(
      (t) =>
        t.title?.toLowerCase().includes(q) ||
        t.description?.toLowerCase().includes(q) ||
        t.category?.toLowerCase().includes(q) ||
        t.targets?.some((target) => target.toLowerCase().includes(q)),
    );
  }, [webTemplates, search]);

  return (
    <div className="flex h-full flex-col">
      <div className="border-border border-b">
        <div className="flex h-12 min-w-0 items-center justify-between py-2 pr-3 pl-3">
          <div className="flex items-center gap-2">
            {isSidebarCollapsed && (
              <Button
                onClick={onExpandSidebar}
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-neutral-600 hover:text-black"
              >
                <Star size={16} className="text-amber-500" />
              </Button>
            )}
            <h3 className="text-sm font-medium">Templates</h3>
          </div>
          <button
            onClick={onCreateTemplate}
            className={cn([
              "rounded-full px-2 py-1.5",
              "bg-linear-to-l from-stone-600 to-stone-500",
              "shadow-[inset_0px_-1px_8px_0px_rgba(41,37,36,1.00)]",
              "shadow-[inset_0px_1px_8px_0px_rgba(120,113,108,1.00)]",
              "flex items-center justify-center gap-1",
              "transition-colors hover:from-stone-700 hover:to-stone-600",
            ])}
          >
            <Plus className="h-4 w-4 text-stone-50" />
            <span className="font-serif text-xs font-medium text-stone-50">
              Create your own template
            </span>
          </button>
        </div>
      </div>

      <div className="relative flex-1 overflow-y-auto">
        <div className="pointer-events-none absolute top-0 right-0 left-0 z-10 h-8 bg-linear-to-b from-white to-transparent" />
        <div className="pointer-events-none absolute right-0 bottom-0 left-0 z-10 h-8 bg-linear-to-t from-white to-transparent" />

        <div className="flex flex-col items-center justify-center gap-8 px-4 py-12">
          <div className="flex max-w-md flex-col items-center justify-start gap-4">
            <h1 className="font-serif text-2xl font-semibold">Templates</h1>
            <p className="text-center text-base text-neutral-600">
              Templates act as AI instructions for each meeting type, giving you
              structured notes instantly
            </p>
          </div>
          <div
            className={cn([
              "h-10 w-80 rounded-lg bg-white px-4",
              "border border-neutral-200",
              "flex items-center gap-2",
              "transition-colors focus-within:border-neutral-400",
            ])}
          >
            <Search className="h-4 w-4 text-neutral-400" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search for a template..."
              className="flex-1 bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className="rounded-xs p-0.5 hover:bg-neutral-100"
              >
                <X className="h-3 w-3 text-neutral-400" />
              </button>
            )}
          </div>
        </div>

        <div className="px-3 pb-8">
          {isWebLoading ? (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {[0, 1, 2, 3, 4, 5].map((i) => (
                <div
                  key={i}
                  className="animate-pulse overflow-hidden rounded-xs border border-stone-100"
                >
                  <div className="h-20 bg-stone-200" />
                  <div className="flex flex-col gap-3 p-3">
                    <div className="h-4 w-3/4 rounded-xs bg-stone-200" />
                    <div className="h-3 w-full rounded-xs bg-stone-100" />
                    <div className="flex gap-2">
                      <div className="h-7 w-16 rounded-3xl bg-stone-100" />
                      <div className="h-7 w-20 rounded-3xl bg-stone-100" />
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : filteredTemplates.length === 0 ? (
            <div className="py-12 text-center text-neutral-500">
              <BookText size={48} className="mx-auto mb-3 text-neutral-300" />
              <p className="text-sm">
                {search ? "No templates found" : "No templates available"}
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {filteredTemplates.map((template, index) => (
                <TemplateCard
                  key={template.slug || index}
                  template={template}
                  onClick={() => {
                    const originalIndex = webTemplates.findIndex(
                      (t) => t.slug === template.slug,
                    );
                    onSelectWebTemplate(
                      originalIndex !== -1 ? originalIndex : index,
                    );
                  }}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function TemplateCard({
  template,
  onClick,
}: {
  template: WebTemplate;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn([
        "w-full overflow-hidden rounded-xs border border-stone-100 text-left",
        "transition-all hover:border-stone-300 hover:shadow-xs",
        "flex flex-col",
      ])}
    >
      <div className="flex h-20 items-center justify-center bg-linear-to-br from-stone-100 to-stone-200">
        <BookText className="h-8 w-8 text-stone-400" />
      </div>
      <div className="flex flex-1 flex-col gap-3 p-3">
        <div className="line-clamp-1 font-serif text-base font-medium">
          {template.title || "Untitled"}
        </div>
        <div className="truncate text-sm text-stone-600">
          {template.description || "No description"}
        </div>
        {template.targets && template.targets.length > 0 && (
          <div className="truncate text-xs text-stone-400">
            {template.targets.join(", ")}
          </div>
        )}
      </div>
    </button>
  );
}

function TemplateListColumn({
  showHomepage,
  isWebMode,
  setIsWebMode,
  userTemplates,
  webTemplates,
  isWebLoading,
  selectedMineId,
  selectedWebIndex,
  setSelectedMineId,
  setSelectedWebIndex,
  setShowHomepage,
  onCreateTemplate,
}: {
  showHomepage: boolean;
  isWebMode: boolean;
  setIsWebMode: (value: boolean) => void;
  userTemplates: UserTemplate[];
  webTemplates: WebTemplate[];
  isWebLoading: boolean;
  selectedMineId: string | null;
  selectedWebIndex: number | null;
  setSelectedMineId: (id: string | null) => void;
  setSelectedWebIndex: (index: number | null) => void;
  setShowHomepage: (value: boolean) => void;
  onCreateTemplate: () => void;
}) {
  const [search, setSearch] = useState("");
  const [sortOption, setSortOption] = useState<SortOption>("alphabetical");

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
      (t) =>
        t.title?.toLowerCase().includes(q) ||
        t.description?.toLowerCase().includes(q),
    );
  }, [sortedUserTemplates, search]);

  const filteredWeb = useMemo(() => {
    if (!search.trim()) return webTemplates;
    const q = search.toLowerCase();
    return webTemplates.filter(
      (t) =>
        t.title?.toLowerCase().includes(q) ||
        t.description?.toLowerCase().includes(q) ||
        t.category?.toLowerCase().includes(q),
    );
  }, [webTemplates, search]);

  if (showHomepage) {
    return (
      <div className="flex h-full w-full flex-col">
        <div className="border-b border-neutral-200">
          <div className="flex h-12 items-center justify-between py-2 pr-1 pl-3">
            <div className="flex items-center gap-2">
              <Star size={16} className="text-amber-500" />
              <span className="text-sm font-medium">Favorites</span>
            </div>
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
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={() => setSortOption("alphabetical")}>
                  A to Z
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => setSortOption("reverse-alphabetical")}
                >
                  Z to A
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
          <div className="flex h-10 items-center gap-2 border-t border-neutral-200 bg-white px-3">
            <Search className="h-4 w-4 text-neutral-400" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearch("");
                }
              }}
              placeholder="Search..."
              className="w-full bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className="rounded-xs p-1 hover:bg-neutral-100"
              >
                <X className="h-4 w-4 text-neutral-400" />
              </button>
            )}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          {filteredMine.length === 0 ? (
            <div className="py-8 text-center text-neutral-500">
              <Star size={32} className="mx-auto mb-2 text-neutral-300" />
              <p className="text-sm">
                {search ? "No templates found" : "No favorites yet"}
              </p>
              {!search && (
                <button
                  onClick={onCreateTemplate}
                  className="mt-3 text-sm text-neutral-600 underline hover:text-neutral-800"
                >
                  Create your first template
                </button>
              )}
            </div>
          ) : (
            filteredMine.map((item) => (
              <button
                key={item.id}
                onClick={() => setSelectedMineId(item.id)}
                className={cn([
                  "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
                  "border-transparent",
                ])}
              >
                <div className="flex items-center gap-2">
                  <BookText className="h-4 w-4 shrink-0 text-neutral-500" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium">
                      {item.title?.trim() || "Untitled"}
                    </div>
                    {item.description && (
                      <div className="truncate text-xs text-neutral-500">
                        {item.description}
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

  const items = isWebMode ? filteredWeb : filteredMine;

  return (
    <div className="flex h-full w-full flex-col">
      <div className="border-b border-neutral-200">
        <div className="flex h-12 items-center justify-between py-2 pr-1 pl-3">
          <button
            onClick={() => setShowHomepage(true)}
            className="text-sm font-medium hover:text-neutral-600"
          >
            Templates
          </button>
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
        </div>
        <div className="flex h-10 items-center gap-2 border-t border-neutral-200 bg-white px-3">
          <Search className="h-4 w-4 text-neutral-400" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setSearch("");
              }
            }}
            placeholder="Search..."
            className="w-full bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
          />
          {search && (
            <button
              onClick={() => setSearch("")}
              className="rounded-xs p-1 hover:bg-neutral-100"
            >
              <X className="h-4 w-4 text-neutral-400" />
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-2">
        {isWebMode && isWebLoading ? (
          <div className="flex flex-col gap-2">
            {[0, 1, 2, 3].map((i) => (
              <div key={i} className="animate-pulse rounded-md px-3 py-2">
                <div className="h-4 w-3/4 rounded-xs bg-neutral-200" />
                <div className="mt-1.5 h-3 w-1/2 rounded-xs bg-neutral-100" />
              </div>
            ))}
          </div>
        ) : items.length === 0 ? (
          <div className="py-8 text-center text-neutral-500">
            <BookText size={32} className="mx-auto mb-2 text-neutral-300" />
            <p className="text-sm">
              {search
                ? "No templates found"
                : isWebMode
                  ? "No community templates"
                  : "No templates yet"}
            </p>
          </div>
        ) : isWebMode ? (
          filteredWeb.map((item, index) => (
            <button
              key={`web-${index}`}
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
                    {item.title || "Untitled"}
                    {item.category && (
                      <span className="ml-1 font-mono text-xs text-stone-400">
                        ({item.category})
                      </span>
                    )}
                  </div>
                  {item.description && (
                    <div className="truncate text-xs text-neutral-500">
                      {item.description}
                    </div>
                  )}
                </div>
              </div>
            </button>
          ))
        ) : (
          filteredMine.map((item) => (
            <button
              key={item.id}
              onClick={() => setSelectedMineId(item.id)}
              className={cn([
                "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
                selectedMineId === item.id
                  ? "border-neutral-500 bg-neutral-100"
                  : "border-transparent",
              ])}
            >
              <div className="flex items-center gap-2">
                <BookText className="h-4 w-4 shrink-0 text-neutral-500" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">
                    {item.title?.trim() || "Untitled"}
                  </div>
                  {item.description && (
                    <div className="truncate text-xs text-neutral-500">
                      {item.description}
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
