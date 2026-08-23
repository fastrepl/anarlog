import { useLingui } from "@lingui/react/macro";
import {
  ArrowClockwise,
  CaretRight,
  Heart,
  MagnifyingGlass,
  Plus,
  X,
} from "@phosphor-icons/react";
import { useCallback, useMemo, useRef, useState } from "react";

import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { cn } from "@anlg/utils";

import { useWebResources } from "~/shared/ui/resource-list";
import {
  DEFAULT_TEMPLATE_ICON,
  filterWebTemplatesAgainstUserTemplates,
  parseWebTemplates,
  TemplateIconGlyph,
  type TemplateIcon,
  useCreateTemplate,
  useOpenTemplatesTab,
  useUserTemplates,
  type WebTemplate,
} from "~/templates";

export type TemplateSelection = {
  templateId: string | null;
  title: string;
};

export function TemplatePickerPopover({
  onSelectTemplate,
  usedTemplateId,
  onRegenerateUsed,
  isRegenerating = false,
  trigger,
}: {
  onSelectTemplate: (selection: TemplateSelection) => void;
  usedTemplateId?: string | null;
  onRegenerateUsed?: () => void;
  isRegenerating?: boolean;
  trigger: React.ReactNode;
}) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const resultRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate("session_note");
  const { data: rawWebTemplates = [] } =
    useWebResources<Record<string, unknown>>("templates");
  const webTemplates = useMemo(
    () =>
      filterWebTemplatesAgainstUserTemplates({
        userTemplates,
        webTemplates: parseWebTemplates(rawWebTemplates),
      }),
    [rawWebTemplates, userTemplates],
  );
  const openTemplatesTab = useOpenTemplatesTab();

  const handleUseTemplate = useCallback(
    (selection: TemplateSelection) => {
      setOpen(false);
      setSearch("");
      resultRefs.current = [];

      onSelectTemplate(selection);
    },
    [onSelectTemplate],
  );

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setSearch("");
      resultRefs.current = [];
    }
  }, []);

  const handleWebTemplateClick = useCallback(
    (template: WebTemplate) => {
      setOpen(false);
      setSearch("");
      resultRefs.current = [];

      void (async () => {
        const templateId = await createTemplate({
          title: template.title,
          description: template.description,
          category: template.category,
          icon: template.icon,
          targets: template.targets,
          sections: template.sections ?? [],
        });
        if (!templateId) {
          return;
        }

        onSelectTemplate({
          templateId,
          title: template.title || "Untitled",
        });
      })();
    },
    [createTemplate, onSelectTemplate],
  );

  const handleCreateTemplate = useCallback(
    (title?: string) => {
      const nextTitle = title?.trim() || "New Template";

      setOpen(false);
      setSearch("");
      resultRefs.current = [];

      void (async () => {
        const templateId = await createTemplate({
          title: nextTitle,
          description: "",
          sections: [],
        });
        if (!templateId) {
          return;
        }

        openTemplatesTab({
          selectedMineId: templateId,
          selectedWebIndex: null,
          isWebMode: false,
          showHomepage: false,
        });
      })();
    },
    [createTemplate, openTemplatesTab],
  );
  const handleSeeAllTemplates = useCallback(() => {
    setOpen(false);
    setSearch("");
    resultRefs.current = [];
    openTemplatesTab({
      showHomepage: false,
      isWebMode: true,
      selectedMineId: null,
      selectedWebIndex: 0,
    });
  }, [openTemplatesTab]);

  const trimmedSearch = search.trim();
  const searchQuery = search.trim().toLowerCase();
  const favoriteTemplates = useMemo(
    () => sortFavoriteTemplates(userTemplates),
    [userTemplates],
  );
  const otherTemplates = useMemo(
    () => sortOtherTemplates(userTemplates),
    [userTemplates],
  );

  const filteredFavoriteTemplates = useMemo(() => {
    if (!searchQuery) {
      return favoriteTemplates;
    }

    return favoriteTemplates.filter((template) =>
      matchesTemplateSearch(template, searchQuery),
    );
  }, [favoriteTemplates, searchQuery]);

  const filteredOtherTemplates = useMemo(() => {
    if (!searchQuery) {
      return otherTemplates;
    }

    return otherTemplates.filter((template) =>
      matchesTemplateSearch(template, searchQuery),
    );
  }, [otherTemplates, searchQuery]);

  const hasSearch = searchQuery.length > 0;
  const filteredWebTemplates = useMemo(() => {
    if (!searchQuery) {
      return webTemplates;
    }

    return webTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(searchQuery) ||
        template.description?.toLowerCase().includes(searchQuery) ||
        template.category?.toLowerCase().includes(searchQuery) ||
        template.targets?.some((target) =>
          target.toLowerCase().includes(searchQuery),
        ),
    );
  }, [searchQuery, webTemplates]);
  const templateItems = useMemo<
    Array<{
      key: string;
      title: string;
      icon: TemplateIcon;
      isFavorite?: boolean;
      onClick: () => void;
    }>
  >(() => {
    const favoriteItems = filteredFavoriteTemplates.map((template) => ({
      key: template.id,
      title: template.title || "Untitled",
      icon: template.icon,
      isFavorite: true,
      onClick: () =>
        handleUseTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
    }));

    const userItems = filteredOtherTemplates.map((template) => ({
      key: template.id,
      title: template.title || "Untitled",
      icon: template.icon,
      onClick: () =>
        handleUseTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
    }));

    const webItems = filteredWebTemplates.map((template, index) => ({
      key: template.slug || `library-${index}`,
      title: template.title || "Untitled",
      icon: template.icon,
      onClick: () => handleWebTemplateClick(template),
    }));

    const otherItems = [...userItems, ...webItems].sort((a, b) =>
      a.title.localeCompare(b.title),
    );

    return [...favoriteItems, ...otherItems];
  }, [
    filteredFavoriteTemplates,
    filteredOtherTemplates,
    filteredWebTemplates,
    handleWebTemplateClick,
    handleUseTemplate,
  ]);
  const resultSections = useMemo<
    Array<{
      key: string;
      title: string;
      icon?: React.ReactNode;
      uppercase?: boolean;
      showHeader?: boolean;
      emptyMessage?: string;
      items: Array<{
        key: string;
        title: string;
        icon: TemplateIcon;
        isFavorite?: boolean;
        onClick: () => void;
      }>;
    }>
  >(() => {
    const autoSection = {
      key: "auto",
      title: "Auto",
      showHeader: false,
      items: [
        {
          key: "auto",
          title: "Auto",
          icon: {
            type: "icon",
            value: "sparkles",
            color: "#9ca3af",
          } satisfies TemplateIcon,
          onClick: () =>
            handleUseTemplate({
              templateId: null,
              title: "Auto",
            }),
        },
      ],
    };

    if (!hasSearch) {
      return [
        autoSection,
        {
          key: "templates",
          title: "Templates",
          showHeader: false,
          items: templateItems,
          emptyMessage: "No templates yet",
        },
      ];
    }

    return [
      autoSection,
      {
        key: "create",
        title: "Create new template",
        icon: <Plus className="h-3.5 w-3.5 text-blue-500" />,
        uppercase: false,
        items: [
          {
            key: `create-${trimmedSearch}`,
            title: trimmedSearch,
            icon: DEFAULT_TEMPLATE_ICON,
            onClick: () => handleCreateTemplate(trimmedSearch),
          },
        ],
      },
      ...(templateItems.length > 0
        ? [
            {
              key: "templates",
              title: "Templates",
              showHeader: false,
              items: templateItems,
            },
          ]
        : []),
    ];
  }, [
    handleCreateTemplate,
    handleUseTemplate,
    hasSearch,
    templateItems,
    trimmedSearch,
  ]);
  const navigableResults = useMemo(
    () => resultSections.flatMap((section) => section.items),
    [resultSections],
  );
  const focusSearchInput = useCallback(() => {
    searchInputRef.current?.focus();
  }, []);
  const focusResult = useCallback((index: number) => {
    resultRefs.current[index]?.focus();
  }, []);
  const handleSearchInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (navigableResults.length === 0) {
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        focusResult(0);
      }

      if (e.key === "ArrowUp") {
        e.preventDefault();
        focusResult(navigableResults.length - 1);
      }
    },
    [focusResult, navigableResults.length],
  );
  const handleResultKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        focusResult(Math.min(index + 1, navigableResults.length - 1));
      }

      if (e.key === "ArrowUp") {
        e.preventDefault();
        if (index === 0) {
          focusSearchInput();
          return;
        }

        focusResult(index - 1);
      }
    },
    [focusResult, focusSearchInput, navigableResults.length],
  );
  let resultIndex = 0;

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent variant="app" className="w-80" align="start">
        <div className="flex flex-col gap-1">
          <AppFloatingPanel className="flex flex-col overflow-hidden">
            <div className="border-border border-b py-1">
              <div
                className={cn([
                  "flex h-8 items-center gap-2 rounded-md px-2.5",
                ])}
              >
                <MagnifyingGlass className="text-muted-foreground h-4 w-4" />
                <input
                  ref={searchInputRef}
                  autoFocus
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  onKeyDown={handleSearchInputKeyDown}
                  placeholder={t`Search templates...`}
                  className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm focus:outline-hidden"
                />
                {search && (
                  <button
                    onClick={() => setSearch("")}
                    className="hover:bg-accent rounded-xs p-0.5"
                  >
                    <X className="text-muted-foreground h-3 w-3" />
                  </button>
                )}
              </div>
            </div>

            <div className="relative">
              <div
                className={cn(["scroll-fade-y max-h-80 overflow-y-auto p-1.5"])}
              >
                <div className="flex flex-col gap-0">
                  {resultSections.map((section) => (
                    <TemplateSection
                      key={section.key}
                      title={section.title}
                      icon={section.icon}
                      uppercase={section.uppercase}
                      showHeader={section.showHeader}
                    >
                      {section.items.length > 0 ? (
                        section.items.map((item) => {
                          const itemIndex = resultIndex;
                          resultIndex += 1;

                          const isUsedTemplate =
                            usedTemplateId !== undefined &&
                            (item.key === "auto"
                              ? usedTemplateId === null
                              : item.key === usedTemplateId);

                          return (
                            <TemplateResultButton
                              key={item.key}
                              buttonRef={(node) => {
                                resultRefs.current[itemIndex] = node;
                              }}
                              title={item.title}
                              icon={item.icon}
                              isFavorite={item.isFavorite}
                              onClick={item.onClick}
                              onKeyDown={(e) =>
                                handleResultKeyDown(e, itemIndex)
                              }
                              regenerateLabel={
                                isUsedTemplate && onRegenerateUsed
                                  ? t`Regenerate`
                                  : undefined
                              }
                              isRegenerating={isRegenerating}
                              onRegenerate={
                                isUsedTemplate ? onRegenerateUsed : undefined
                              }
                            />
                          );
                        })
                      ) : (
                        <div className="text-muted-foreground px-2 py-3 text-sm">
                          {section.emptyMessage}
                        </div>
                      )}
                    </TemplateSection>
                  ))}
                </div>
              </div>
            </div>
          </AppFloatingPanel>

          <button
            onClick={handleSeeAllTemplates}
            className={cn([
              "flex h-7 w-full items-center justify-center gap-1 rounded-lg px-3 text-xs font-medium",
              "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
            ])}
          >
            {t`See all templates`}
            <CaretRight className="h-3.5 w-3.5" />
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function matchesTemplateSearch(
  template: {
    title?: string;
    description?: string;
    category?: string;
    targets?: string[];
  },
  query: string,
) {
  return (
    template.title?.toLowerCase().includes(query) ||
    template.description?.toLowerCase().includes(query) ||
    template.category?.toLowerCase().includes(query) ||
    template.targets?.some((target) => target.toLowerCase().includes(query))
  );
}

function sortFavoriteTemplates<
  T extends { pinned?: boolean; pinOrder?: number; title?: string },
>(templates: T[]) {
  return [...templates]
    .filter((template) => template.pinned)
    .sort((a, b) => {
      const orderA = a.pinOrder ?? Infinity;
      const orderB = b.pinOrder ?? Infinity;
      if (orderA !== orderB) {
        return orderA - orderB;
      }
      return (a.title || "").localeCompare(b.title || "");
    });
}

function sortOtherTemplates<T extends { pinned?: boolean; title?: string }>(
  templates: T[],
) {
  return [...templates]
    .filter((template) => !template.pinned)
    .sort((a, b) => (a.title || "").localeCompare(b.title || ""));
}

function TemplateSection({
  title,
  children,
  icon,
  uppercase = true,
  showHeader = true,
}: {
  title: string;
  children: React.ReactNode;
  icon?: React.ReactNode;
  uppercase?: boolean;
  showHeader?: boolean;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      {showHeader ? (
        <div className="flex items-center gap-2 px-2">
          {icon}
          <p
            className={cn([
              "text-muted-foreground font-mono text-[11px] font-medium tracking-wide",
              uppercase && "uppercase",
            ])}
          >
            {title}
          </p>
        </div>
      ) : null}
      <div className="flex flex-col gap-0">{children}</div>
    </div>
  );
}

function TemplateResultButton({
  buttonRef,
  title,
  icon,
  isFavorite = false,
  onClick,
  onKeyDown,
  regenerateLabel,
  isRegenerating = false,
  onRegenerate,
}: {
  buttonRef?: React.Ref<HTMLButtonElement>;
  title: string;
  icon: TemplateIcon;
  isFavorite?: boolean;
  onClick: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLButtonElement>) => void;
  regenerateLabel?: string;
  isRegenerating?: boolean;
  onRegenerate?: () => void;
}) {
  return (
    <div
      className={cn([
        "hover:bg-accent focus-within:bg-muted h-8 w-full rounded-md px-2.5",
        "flex items-center gap-1.5",
      ])}
    >
      <button
        ref={buttonRef}
        className="flex min-w-0 flex-1 items-center gap-1.5 text-left focus:outline-hidden"
        onClick={onClick}
        onKeyDown={onKeyDown}
      >
        <TemplateIconGlyph icon={icon} className="size-4 text-sm" />
        <span className="text-foreground min-w-0 truncate text-sm font-medium">
          {title}
        </span>
        {isFavorite ? (
          <Heart
            aria-hidden
            className="size-3.5 shrink-0 fill-rose-500 text-rose-500"
          />
        ) : null}
      </button>
      {regenerateLabel && onRegenerate ? (
        <button
          type="button"
          aria-label={regenerateLabel}
          disabled={isRegenerating}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onRegenerate();
          }}
          className={cn([
            "text-muted-foreground hover:text-foreground inline-flex shrink-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium",
            isRegenerating ? "cursor-not-allowed opacity-70" : "cursor-pointer",
          ])}
        >
          <ArrowClockwise
            className={cn(["size-3", isRegenerating && "animate-spin"])}
          />
          {regenerateLabel}
        </button>
      ) : null}
    </div>
  );
}
