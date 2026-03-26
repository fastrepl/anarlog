import { BookText, Plus, Search, Star, X } from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useUserTemplates } from "~/templates";

type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  sections: Array<{ title: string; description: string }>;
};

export function TemplatesContent() {
  const [search, setSearch] = useState("");
  const userTemplates = useUserTemplates();
  const { data: webTemplates = [], isLoading: isWebLoading } =
    useWebResources<WebTemplate>("templates");
  const openNew = useTabs((state) => state.openNew);

  const filteredUser = useMemo(() => {
    if (!search.trim()) return userTemplates;
    const q = search.toLowerCase();
    return userTemplates.filter(
      (t) =>
        t.title?.toLowerCase().includes(q) ||
        t.description?.toLowerCase().includes(q),
    );
  }, [userTemplates, search]);

  const filteredWeb = useMemo(() => {
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

  const { user_id } = main.UI.useValues(main.STORE_ID);

  const setRow = main.UI.useSetRowCallback(
    "templates",
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      description: string;
      sections: Array<{ title: string; description: string }>;
    }) => p.id,
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      description: string;
      sections: Array<{ title: string; description: string }>;
    }) => ({
      user_id: p.user_id,
      title: p.title,
      description: p.description,
      sections: JSON.stringify(p.sections),
    }),
    [],
    main.STORE_ID,
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
  }, [user_id, setRow]);

  const handleOpenUserTemplate = useCallback(
    (id: string) => {
      openNew({
        type: "templates",
        state: {
          selectedMineId: id,
          selectedWebIndex: null,
          isWebMode: false,
          showHomepage: false,
        },
      });
    },
    [openNew],
  );

  const handleOpenWebTemplate = useCallback(
    (index: number) => {
      openNew({
        type: "templates",
        state: {
          selectedMineId: null,
          selectedWebIndex: index,
          isWebMode: true,
          showHomepage: false,
        },
      });
    },
    [openNew],
  );

  return (
    <div className="flex flex-col gap-4 pt-2">
      <div className="flex items-center gap-2">
        <div
          className={cn([
            "h-9 flex-1 rounded-lg bg-white px-3",
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
            placeholder="Search templates..."
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
        <button
          onClick={handleCreateTemplate}
          className={cn([
            "h-9 rounded-lg px-3",
            "bg-linear-to-l from-stone-600 to-stone-500",
            "shadow-[inset_0px_-1px_8px_0px_rgba(41,37,36,1.00)]",
            "shadow-[inset_0px_1px_8px_0px_rgba(120,113,108,1.00)]",
            "flex items-center gap-1.5",
            "transition-colors hover:from-stone-700 hover:to-stone-600",
          ])}
        >
          <Plus className="h-4 w-4 text-stone-50" />
          <span className="text-xs font-medium text-stone-50">New</span>
        </button>
      </div>

      {filteredUser.length > 0 && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <Star size={14} className="text-amber-500" />
            <h3 className="text-xs font-medium tracking-wide text-neutral-500 uppercase">
              Favorites
            </h3>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {filteredUser.map((template) => (
              <TemplateCardItem
                key={template.id}
                title={template.title || "Untitled"}
                description={template.description}
                onClick={() => handleOpenUserTemplate(template.id)}
              />
            ))}
          </div>
        </div>
      )}

      <div className="flex flex-col gap-2">
        <h3 className="text-xs font-medium tracking-wide text-neutral-500 uppercase">
          Suggestions
        </h3>
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
                </div>
              </div>
            ))}
          </div>
        ) : filteredWeb.length === 0 ? (
          <div className="py-8 text-center text-neutral-500">
            <BookText size={32} className="mx-auto mb-2 text-neutral-300" />
            <p className="text-sm">
              {search ? "No templates found" : "No suggestions available"}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {filteredWeb.map((template, index) => (
              <TemplateCardItem
                key={template.slug || index}
                title={template.title || "Untitled"}
                description={template.description}
                targets={template.targets}
                onClick={() => {
                  const originalIndex = webTemplates.findIndex(
                    (t) => t.slug === template.slug,
                  );
                  handleOpenWebTemplate(
                    originalIndex !== -1 ? originalIndex : index,
                  );
                }}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function TemplateCardItem({
  title,
  description,
  targets,
  onClick,
}: {
  title: string;
  description?: string;
  targets?: string[];
  onClick?: () => void;
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
          {title}
        </div>
        <div className="truncate text-sm text-stone-600">
          {description || "No description"}
        </div>
        {targets && targets.length > 0 && (
          <div className="truncate text-xs text-stone-400">
            {targets.join(", ")}
          </div>
        )}
      </div>
    </button>
  );
}
