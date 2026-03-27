import { BookText, Plus, Search, X } from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import type { TemplateSection } from "@hypr/store";
import { cn } from "@hypr/utils";

import { TemplateDetailsColumn } from "./components/details";
import {
  useCreateTemplate,
  useUserTemplates,
  type WebTemplate,
} from "./shared";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export { useUserTemplates } from "./shared";

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

function TemplateView({ tab }: { tab: Extract<Tab, { type: "templates" }> }) {
  const updateTabState = useTabs((state) => state.updateTemplatesTabState);
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
  const { data: webTemplates = [], isLoading: isWebLoading } =
    useWebResources<WebTemplate>("templates");

  const { selectedMineId, selectedWebIndex } = tab.state;
  const showHomepage = tab.state.showHomepage ?? true;
  const isWebMode = tab.state.isWebMode ?? userTemplates.length === 0;

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
    (templateId: string) => templateId,
    main.STORE_ID,
  );

  const handleDeleteTemplate = useCallback(
    (id: string) => {
      deleteTemplateFromStore(id);
      setSelectedMineId(null);
    },
    [deleteTemplateFromStore, setSelectedMineId],
  );

  const handleCloneTemplate = useCallback(
    (template: {
      title: string;
      description: string;
      sections: TemplateSection[];
    }) => {
      const id = createTemplate(template);
      if (id) {
        setSelectedMineId(id);
      }
    },
    [createTemplate, setSelectedMineId],
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

  return (
    <div className="h-full">
      {showHomepage ? (
        <TemplatesHomepage
          webTemplates={webTemplates}
          isWebLoading={isWebLoading}
          onSelectWebTemplate={setSelectedWebIndex}
          onCreateTemplate={handleCreateTemplate}
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
    </div>
  );
}

function TemplatesHomepage({
  webTemplates,
  isWebLoading,
  onSelectWebTemplate,
  onCreateTemplate,
}: {
  webTemplates: WebTemplate[];
  isWebLoading: boolean;
  onSelectWebTemplate: (index: number) => void;
  onCreateTemplate: () => void;
}) {
  const [search, setSearch] = useState("");

  const filteredTemplates = useMemo(() => {
    if (!search.trim()) return webTemplates;
    const query = search.toLowerCase();
    return webTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(query) ||
        template.description?.toLowerCase().includes(query) ||
        template.category?.toLowerCase().includes(query) ||
        template.targets?.some((target) =>
          target.toLowerCase().includes(query),
        ),
    );
  }, [webTemplates, search]);

  return (
    <div className="flex h-full flex-col">
      <div className="border-border border-b">
        <div className="flex h-12 min-w-0 items-center justify-between py-2 pr-3 pl-3">
          <h3 className="text-sm font-medium">Templates</h3>
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
              {[0, 1, 2, 3, 4, 5].map((index) => (
                <div
                  key={index}
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
                      (entry) => entry.slug === template.slug,
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
