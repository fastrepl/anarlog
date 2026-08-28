import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsDownUp,
  BookOpenText,
  MagnifyingGlass,
  Plus,
  Sparkle,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import { type WebTemplate } from "./codec";
import { getTemplateCopyTitle, type UserTemplate } from "./queries";
import { TemplateIconGlyph } from "./template-icon";
import { AUTO_TEMPLATE_ID, useTemplateTab } from "./utils";

import { useConfigValue } from "~/shared/config";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { CustomSidebarHeader } from "~/sidebar/custom-sidebar-header";
import { type Tab } from "~/store/zustand/tabs";

type SortOption = "alphabetical" | "reverse-alphabetical";

export function TemplatesSidebarContent({
  tab,
}: {
  tab: Extract<Tab, { type: "templates" }>;
}) {
  const { t } = useLingui();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState("");
  const [sortOption, setSortOption] = useState<SortOption>("alphabetical");
  const autoPrompt = useConfigValue("auto_summary_prompt");

  const {
    userTemplates,
    webTemplates,
    isWebLoading,
    isWebMode,
    selectedMineId: effectiveSelectedMineId,
    selectedWebIndex: effectiveSelectedWebIndex,
    setSelectedMineId,
    setSelectedWebIndex,
    createTemplate,
    createDefaultTemplate,
    deleteTemplate,
    toggleTemplateFavorite,
  } = useTemplateTab(tab);

  const handleDuplicateTemplate = useCallback(
    async (template: UserTemplate) => {
      const id = await createTemplate({
        title: getTemplateCopyTitle(template.title),
        description: template.description ?? "",
        category: template.category,
        icon: template.icon,
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
    async (id: string) => {
      await deleteTemplate(id);

      if (effectiveSelectedMineId === id) {
        setSelectedMineId(null);
      }
    },
    [deleteTemplate, effectiveSelectedMineId, setSelectedMineId],
  );

  const handleToggleFavorite = useCallback(
    async (id: string) => {
      await toggleTemplateFavorite(id);
    },
    [toggleTemplateFavorite],
  );

  const sortedUserTemplates = useMemo(() => {
    const favorites = userTemplates
      .filter((template) => template.pinned)
      .sort((a, b) => {
        const orderA = a.pinOrder ?? Infinity;
        const orderB = b.pinOrder ?? Infinity;
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
          key: typeof AUTO_TEMPLATE_ID;
          title: "Auto";
          selected: boolean;
          source: "auto";
          customized: boolean;
        }
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
    const query = search.trim().toLowerCase();
    const auto =
      !query || "auto".includes(query)
        ? [
            {
              key: AUTO_TEMPLATE_ID as typeof AUTO_TEMPLATE_ID,
              title: "Auto" as const,
              selected:
                !isWebMode && effectiveSelectedMineId === AUTO_TEMPLATE_ID,
              source: "auto" as const,
              customized: Boolean(autoPrompt.trim()),
            },
          ]
        : [];
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

    return [...auto, ...mine, ...web];
  }, [
    autoPrompt,
    effectiveSelectedMineId,
    effectiveSelectedWebIndex,
    filteredMine,
    filteredWeb,
    isWebMode,
    search,
  ]);

  const hasResults = combinedTemplates.length > 0;
  const isEmpty = !isWebLoading && !hasResults;

  const selectCombinedTemplate = useCallback(
    (
      item:
        | {
            source: "auto";
          }
        | {
            source: "user";
            template: UserTemplate;
          }
        | {
            source: "web";
            index: number;
          },
    ) => {
      if (item.source === "auto") {
        setSelectedMineId(AUTO_TEMPLATE_ID);
        return;
      }

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
    <div {...stylex.props(styles.root)}>
      <div>
        <CustomSidebarHeader>
          {userTemplates.length > 1 && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button size="icon" variant="ghost" sx={styles.headerButton}>
                  <ArrowsDownUp size={16} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent variant="app" align="end">
                <AppFloatingPanel sx={styles.menuPanel}>
                  <DropdownMenuItem
                    onClick={() => setSortOption("alphabetical")}
                  >
                    <Trans>A to Z</Trans>
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => setSortOption("reverse-alphabetical")}
                  >
                    <Trans>Z to A</Trans>
                  </DropdownMenuItem>
                </AppFloatingPanel>
              </DropdownMenuContent>
            </DropdownMenu>
          )}

          <Button
            size="icon"
            variant="ghost"
            sx={styles.headerButton}
            onClick={createDefaultTemplate}
          >
            <Plus size={16} />
          </Button>
        </CustomSidebarHeader>

        <div {...stylex.props(styles.searchContainer)}>
          <div {...stylex.props(styles.searchField)}>
            <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearch("");
                }
              }}
              placeholder={t`Search templates...`}
              {...stylex.props(styles.searchInput)}
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                {...stylex.props(styles.clearButton)}
                aria-label={t`Clear search`}
              >
                <X {...stylex.props(styles.icon)} />
              </button>
            )}
          </div>
        </div>
      </div>

      <div ref={scrollContainerRef} {...stylex.props(styles.scroller)}>
        {isEmpty ? (
          <div {...stylex.props(styles.emptyState)}>
            <BookOpenText size={32} {...stylex.props(styles.emptyIcon)} />
            <p {...stylex.props(styles.emptyText)}>
              {search ? "No templates found" : "No templates yet"}
            </p>
            {!search && (
              <button
                onClick={createDefaultTemplate}
                {...stylex.props(styles.createButton)}
              >
                Create my first template
              </button>
            )}
          </div>
        ) : (
          <>
            {hasResults && (
              <div {...stylex.props(styles.listSection)}>
                {combinedTemplates.map((item) =>
                  item.source === "auto" ? (
                    <button
                      key={item.key}
                      type="button"
                      onClick={() => setSelectedMineId(AUTO_TEMPLATE_ID)}
                      data-template-selected={item.selected}
                      {...stylex.props([
                        styles.listItem,
                        item.selected
                          ? styles.selectedListItem
                          : styles.unselectedListItem,
                      ])}
                    >
                      <div {...stylex.props(styles.itemContent)}>
                        <Sparkle {...stylex.props(styles.autoIcon)} />
                        <div {...stylex.props(styles.itemIdentity)}>
                          <div {...stylex.props(styles.itemTitle)}>
                            {item.title}
                          </div>
                          {item.customized ? (
                            <div {...stylex.props(styles.itemDescription)}>
                              <Trans>Customized</Trans>
                            </div>
                          ) : null}
                        </div>
                      </div>
                    </button>
                  ) : item.source === "user" ? (
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
                      {...stylex.props([
                        styles.listItem,
                        item.selected
                          ? styles.selectedListItem
                          : styles.unselectedListItem,
                      ])}
                    >
                      <div {...stylex.props(styles.itemContent)}>
                        <TemplateIconGlyph
                          icon={item.template.icon}
                          sx={styles.templateIcon}
                        />
                        <div {...stylex.props(styles.itemIdentity)}>
                          <div {...stylex.props(styles.itemTitle)}>
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
              <div {...stylex.props(styles.listSection)}>
                <div {...stylex.props(styles.skeletonList)}>
                  {[0, 1, 2, 3].map((index) => (
                    <div key={index} {...stylex.props(styles.skeleton)}>
                      <div {...stylex.props(styles.skeletonTitle)} />
                      <div {...stylex.props(styles.skeletonDescription)} />
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
      {...stylex.props([
        styles.listItem,
        selected ? styles.selectedListItem : styles.unselectedListItem,
      ])}
    >
      <div {...stylex.props(styles.itemContent)}>
        <TemplateIconGlyph icon={template.icon} sx={styles.templateIcon} />
        <div {...stylex.props(styles.itemIdentity)}>
          <div {...stylex.props(styles.itemTitle)}>
            {template.title?.trim() || "Untitled"}
          </div>
        </div>
      </div>
    </button>
  );
}

const pulse = stylex.keyframes({
  "0%, 100%": {
    opacity: 1,
  },
  "50%": {
    opacity: 0.5,
  },
});

const styles = stylex.create({
  autoIcon: {
    color: "rgb(139 92 246)",
    height: "1rem",
    width: "1rem",
  },
  clearButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
    flexShrink: 0,
    height: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  createButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginTop: "0.75rem",
    textDecorationLine: "underline",
  },
  emptyIcon: {
    color: `color-mix(in srgb, ${colors.mutedForeground} 70%, transparent)`,
    marginBottom: "0.5rem",
    marginInline: "auto",
  },
  emptyState: {
    color: colors.mutedForeground,
    paddingBlock: "2rem",
    paddingInline: "0.75rem",
    textAlign: "center",
  },
  emptyText: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  headerButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    position: "relative",
    zIndex: 60,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  itemContent: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  itemDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  itemIdentity: {
    flex: "1",
    minWidth: 0,
  },
  itemTitle: {
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  listItem: {
    borderRadius: radii.lg,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
    width: "100%",
  },
  listSection: {
    paddingTop: "0.25rem",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    width: "100%",
  },
  scroller: {
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    flex: "1",
    overflowY: "auto",
    scrollbarWidth: "none",
  },
  searchContainer: {
    paddingBottom: "0.5rem",
  },
  searchField: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in srgb, ${colors.accent} 50%, transparent)`,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexShrink: 0,
    gap: "0.5rem",
    height: "2rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  searchInput: {
    "::placeholder": {
      color: colors.mutedForeground,
      fontSize: "0.875rem",
    },
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    outlineColor: {
      default: null,
      ":focus": "transparent",
    },
    outlineOffset: {
      default: null,
      ":focus": "2px",
    },
    outlineStyle: {
      default: null,
      ":focus": "solid",
    },
    outlineWidth: {
      default: null,
      ":focus": "2px",
    },
  },
  selectedListItem: {
    backgroundColor: colors.accent,
  },
  skeleton: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    borderRadius: radii.lg,
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  skeletonDescription: {
    backgroundColor: colors.muted,
    borderRadius: "0.125rem",
    height: "0.75rem",
    marginTop: "0.375rem",
    width: "33.333333%",
  },
  skeletonList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
  },
  skeletonTitle: {
    backgroundColor: colors.accent,
    borderRadius: "0.125rem",
    height: "1rem",
    width: "75%",
  },
  templateIcon: {
    fontSize: "0.875rem",
    height: "1rem",
    width: "1rem",
  },
  unselectedListItem: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.accent} 50%, transparent)`,
    },
  },
});
