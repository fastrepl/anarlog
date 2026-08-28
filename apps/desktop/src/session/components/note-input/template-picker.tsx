import { useLingui } from "@lingui/react/macro";
import {
  ArrowClockwise,
  CaretRight,
  Heart,
  MagnifyingGlass,
  Plus,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo, useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
        icon: <Plus {...stylex.props(styles.createIcon)} />,
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
      <PopoverContent variant="app" sx={styles.popover} align="start">
        <div {...stylex.props(styles.root)}>
          <AppFloatingPanel sx={styles.panel}>
            <div {...stylex.props(styles.searchFrame)}>
              <div {...stylex.props(styles.searchRow)}>
                <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
                <input
                  ref={searchInputRef}
                  autoFocus
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  onKeyDown={handleSearchInputKeyDown}
                  placeholder={t`Search templates...`}
                  {...stylex.props(styles.searchInput)}
                />
                {search && (
                  <button
                    onClick={() => setSearch("")}
                    {...stylex.props(styles.clearButton)}
                  >
                    <X {...stylex.props(styles.clearIcon)} />
                  </button>
                )}
              </div>
            </div>

            <div {...stylex.props(styles.resultsFrame)}>
              <div {...mergeStyleXProps(styles.resultsScroll, "scroll-fade-y")}>
                <div {...stylex.props(styles.sectionList)}>
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
                        <div {...stylex.props(styles.empty)}>
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
            {...stylex.props(styles.seeAllButton)}
          >
            {t`See all templates`}
            <CaretRight {...stylex.props(styles.smallIcon)} />
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
    <div {...stylex.props(styles.section)}>
      {showHeader ? (
        <div {...stylex.props(styles.sectionHeader)}>
          {icon}
          <p
            {...stylex.props(
              styles.sectionTitle,
              uppercase && styles.uppercase,
            )}
          >
            {title}
          </p>
        </div>
      ) : null}
      <div {...stylex.props(styles.sectionItems)}>{children}</div>
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
    <div {...stylex.props(styles.result)}>
      <button
        ref={buttonRef}
        {...stylex.props(styles.resultMain)}
        onClick={onClick}
        onKeyDown={onKeyDown}
      >
        <TemplateIconGlyph icon={icon} sx={styles.resultIcon} />
        <span {...stylex.props(styles.resultTitle)}>{title}</span>
        {isFavorite ? (
          <Heart aria-hidden {...stylex.props(styles.favoriteIcon)} />
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
          {...stylex.props(
            styles.regenerateButton,
            isRegenerating
              ? styles.regenerateDisabled
              : styles.regenerateEnabled,
          )}
        >
          <ArrowClockwise
            {...stylex.props(
              styles.regenerateIcon,
              isRegenerating && styles.spinning,
            )}
          />
          {regenerateLabel}
        </button>
      ) : null}
    </div>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  clearButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: "0.125rem",
    padding: "0.125rem",
  },
  clearIcon: {
    color: colors.mutedForeground,
    height: "0.75rem",
    width: "0.75rem",
  },
  createIcon: {
    color: "rgb(59 130 246)",
    height: "0.875rem",
    width: "0.875rem",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: "0.75rem",
    paddingInline: "0.5rem",
  },
  favoriteIcon: {
    color: "rgb(244 63 94)",
    fill: "rgb(244 63 94)",
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
  },
  popover: {
    width: "20rem",
  },
  regenerateButton: {
    alignItems: "center",
    borderRadius: radii.md,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "inline-flex",
    flexShrink: 0,
    fontSize: "0.6875rem",
    fontWeight: 500,
    gap: "0.25rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  regenerateDisabled: {
    cursor: "not-allowed",
    opacity: 0.7,
  },
  regenerateEnabled: {
    cursor: "pointer",
  },
  regenerateIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  result: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":focus-within": colors.muted,
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    display: "flex",
    gap: "0.375rem",
    height: "2rem",
    paddingInline: "0.625rem",
    width: "100%",
  },
  resultIcon: {
    fontSize: "0.875rem",
    height: "1rem",
    width: "1rem",
  },
  resultMain: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.375rem",
    minWidth: 0,
    outline: {
      default: null,
      ":focus": "none",
    },
    textAlign: "left",
  },
  resultTitle: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 500,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  resultsFrame: {
    position: "relative",
  },
  resultsScroll: {
    maxHeight: "20rem",
    overflowY: "auto",
    padding: "0.375rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
  },
  searchFrame: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingBlock: "0.25rem",
  },
  searchIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  searchInput: {
    backgroundColor: "transparent",
    color: {
      default: null,
      "::placeholder": colors.mutedForeground,
    },
    flex: "1",
    fontSize: "0.875rem",
    outline: {
      default: null,
      ":focus": "none",
    },
  },
  searchRow: {
    alignItems: "center",
    borderRadius: radii.md,
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    paddingInline: "0.625rem",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  sectionHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    paddingInline: "0.5rem",
  },
  sectionItems: {
    display: "flex",
    flexDirection: "column",
    gap: 0,
  },
  sectionList: {
    display: "flex",
    flexDirection: "column",
    gap: 0,
  },
  sectionTitle: {
    color: colors.mutedForeground,
    fontFamily: fonts.mono,
    fontSize: "0.6875rem",
    fontWeight: 500,
    letterSpacing: "0.025em",
  },
  seeAllButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.lg,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.25rem",
    height: "1.75rem",
    justifyContent: "center",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    width: "100%",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  uppercase: {
    textTransform: "uppercase",
  },
});
