import {
  AlignLeftIcon,
  AudioLinesIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  HeartIcon,
  LightbulbIcon,
  PlusIcon,
  SearchIcon,
  SparklesIcon,
  XIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { json2md, parseJsonContent } from "@hypr/editor/markdown";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import {
  formatTranscriptExportSegments,
  useTranscriptExportSegments,
} from "./transcript/export-data";

import { useAITaskTask } from "~/ai/hooks";
import { useLanguageModel, useLLMConnectionStatus } from "~/ai/hooks";
import { extractPlainText } from "~/search/contexts/engine/utils";
import { getEnhancerService } from "~/services/enhancer";
import { useHasTranscript } from "~/session/components/shared";
import { shouldShowEmptySummaryConfigError } from "~/session/enhance-config";
import { useEnsureDefaultSummary } from "~/session/hooks/useEnhancedNotes";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import {
  filterWebTemplatesAgainstUserTemplates,
  getTemplateCreatorLabel,
  parseWebTemplates,
  useCreateTemplate,
  useTemplateCreatorName,
  useUserTemplate,
  useUserTemplates,
  type WebTemplate,
} from "~/templates";

function getStoredNoteMarkdown(content: string | undefined) {
  const trimmed = content?.trim() ?? "";

  if (!trimmed) {
    return "";
  }

  if (!trimmed.startsWith("{")) {
    return trimmed;
  }

  return json2md(parseJsonContent(trimmed)).trim();
}

const UUID_TITLE_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const ISO_TITLE_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/;

function IconHeaderTab({
  isActive,
  label,
  icon,
  onClick,
  onContextMenu,
  title,
}: {
  isActive: boolean;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  title?: string;
}) {
  return (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={label}
      aria-current={isActive ? "page" : undefined}
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={title}
      className={iconHeaderTabClassName(
        isActive,
        cn(["min-w-10 px-2", isActive ? "max-w-40 gap-1.5" : null]),
      )}
    >
      {icon}
      {isActive && (
        <span className="min-w-0 truncate text-xs font-medium">{label}</span>
      )}
    </button>
  );
}

function iconHeaderTabClassName(isActive: boolean, className?: string) {
  return cn([
    "flex shrink-0 items-center justify-center rounded-full transition-colors [&>svg]:shrink-0",
    isActive
      ? ["bg-foreground/10 text-foreground -my-px h-[30px]"]
      : [
          "h-7",
          "text-muted-foreground/70",
          "hover:bg-background/60 hover:text-foreground",
        ],
    className,
  ]);
}

function getEnhancedNoteTitle({
  rawTitle,
  templateTitle,
  templateId,
}: {
  rawTitle: unknown;
  templateTitle: string | null;
  templateId: string | undefined;
}) {
  if (templateTitle) {
    return templateTitle;
  }

  const title = typeof rawTitle === "string" ? rawTitle.trim() : "";
  if (!title) {
    return "Summary";
  }

  const isGeneratedTitle =
    title === "Summary" ||
    title === templateId ||
    UUID_TITLE_RE.test(title) ||
    ISO_TITLE_RE.test(title);

  if (isGeneratedTitle) {
    return "Summary";
  }

  return title;
}

async function copyTextToClipboard(
  text: string,
  messages?: {
    success: string;
    error: string;
  },
) {
  try {
    try {
      await navigator.clipboard.write([
        new ClipboardItem({
          "text/plain": new Blob([text], {
            type: "text/plain",
          }),
          "text/markdown": new Blob([text], {
            type: "text/markdown",
          }),
        }),
      ]);
    } catch {
      // Fallback for environments that do not support text/markdown
      await navigator.clipboard.writeText(text);
    }

    if (messages) {
      sonnerToast.success(messages.success);
    }

    return true;
  } catch (error) {
    console.error("Failed to copy note tab content", error);

    if (messages) {
      sonnerToast.error(messages.error);
    }

    return false;
  }
}

type TemplateSelection = {
  templateId: string;
  title: string;
};

function HeaderTabRaw({
  isActive,
  onClick = () => {},
  sessionId,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
}) {
  const rawMd = main.UI.useCell(
    "sessions",
    sessionId,
    "raw_md",
    main.STORE_ID,
  ) as string | undefined;
  const memoMarkdown = useMemo(() => getStoredNoteMarkdown(rawMd), [rawMd]);
  const contextMenu = useMemo<MenuItemDef[]>(
    () => [
      {
        id: `copy-memo-${sessionId}`,
        text: "Copy",
        action: () => {
          void copyTextToClipboard(memoMarkdown, {
            success: "Memo copied to clipboard",
            error: "Failed to copy memo",
          });
        },
        disabled: memoMarkdown.length === 0,
      },
    ],
    [memoMarkdown, sessionId],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <IconHeaderTab
      isActive={isActive}
      label="Memos"
      icon={<AlignLeftIcon className="size-4" />}
      onClick={onClick}
      onContextMenu={showContextMenu}
    />
  );
}

function HeaderTabEnhanced({
  isActive,
  onClick = () => {},
  sessionId,
  enhancedNoteId,
  canRemove = false,
  onRemove,
  onSelectNote,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  enhancedNoteId: string;
  canRemove?: boolean;
  onRemove?: () => void;
  onSelectNote?: (enhancedNoteId: string) => void;
}) {
  const { isGenerating, isError, onRegenerate } = useEnhanceLogic(
    sessionId,
    enhancedNoteId,
  );
  const content = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "content",
    main.STORE_ID,
  ) as string | undefined;
  const rawTitle = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "title",
    main.STORE_ID,
  );
  const templateId = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "template_id",
    main.STORE_ID,
  ) as string | undefined;
  const { data: template } = useUserTemplate(templateId);
  const templateTitle = template?.title?.trim() || null;
  const tabTitle = getEnhancedNoteTitle({
    rawTitle,
    templateTitle,
    templateId,
  });
  const noteMarkdown = useMemo(() => getStoredNoteMarkdown(content), [content]);

  const handleCopy = useCallback(() => {
    return copyTextToClipboard(noteMarkdown, {
      success: `${tabTitle} copied to clipboard`,
      error: `Failed to copy ${tabTitle}`,
    });
  }, [noteMarkdown, tabTitle]);
  const handleRegenerate = useCallback(() => {
    void onRegenerate(null);
  }, [onRegenerate]);
  const handleSelectTemplate = useCallback(
    (selection: TemplateSelection) => {
      if (isGenerating) {
        return;
      }

      const result = getEnhancerService()?.enhance(sessionId, {
        templateId: selection.templateId,
        targetNoteId: enhancedNoteId,
        templateTitle: selection.title,
      });

      if (
        (result?.type === "started" || result?.type === "already_active") &&
        result.noteId
      ) {
        onSelectNote?.(result.noteId);
      }
    },
    [enhancedNoteId, isGenerating, onSelectNote, sessionId],
  );
  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const items: MenuItemDef[] = [
      {
        id: `copy-enhanced-${enhancedNoteId}`,
        text: "Copy",
        action: () => {
          void handleCopy();
        },
        disabled: noteMarkdown.length === 0,
      },
      {
        id: `regenerate-enhanced-${enhancedNoteId}`,
        text: "Regenerate",
        action: handleRegenerate,
        disabled: isGenerating,
      },
    ];

    if (canRemove) {
      items.push({ separator: true });
      items.push({
        id: `remove-enhanced-${enhancedNoteId}`,
        text: "Remove",
        action: () => {
          onRemove?.();
        },
        disabled: isGenerating || !onRemove,
      });
    }

    return items;
  }, [
    canRemove,
    enhancedNoteId,
    handleCopy,
    handleRegenerate,
    isGenerating,
    noteMarkdown.length,
    onRemove,
  ]);
  const showContextMenu = useNativeContextMenu(contextMenu);
  const templateTooltip =
    templateId && templateTitle
      ? `${templateTitle} was used to generate this summary.`
      : undefined;

  const templateMenuTrigger = (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={tabTitle}
      aria-current="page"
      aria-disabled={isGenerating}
      tabIndex={isGenerating ? -1 : 0}
      onClick={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={showContextMenu}
      title={templateTooltip}
      className={iconHeaderTabClassName(
        true,
        cn([
          "max-w-56 min-w-[62px] gap-1.5 pr-1.5 pl-2",
          isGenerating ? "cursor-not-allowed opacity-70" : "cursor-pointer",
          isError
            ? [
                "text-red-600 hover:bg-red-50 hover:text-red-700 focus-visible:bg-red-50",
                "dark:text-red-400 dark:hover:bg-red-950/50 dark:hover:text-red-300 dark:focus-visible:bg-red-950/50",
              ]
            : "focus-visible:bg-background focus-visible:text-foreground",
        ]),
      )}
    >
      {isGenerating ? (
        <Spinner size={16} className="shrink-0" />
      ) : (
        <SparklesIcon className="size-4" />
      )}
      <span className="min-w-0 truncate text-xs font-medium">{tabTitle}</span>
      <ChevronDownIcon className="size-3.5" />
    </button>
  );

  return isActive ? (
    <TemplatePickerPopover
      sessionId={sessionId}
      onSelectTemplate={handleSelectTemplate}
      trigger={templateMenuTrigger}
    />
  ) : (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={tabTitle}
      onClick={onClick}
      onContextMenu={showContextMenu}
      title={templateTooltip}
      className={iconHeaderTabClassName(false, "min-w-10 px-2")}
    >
      {isGenerating ? (
        <Spinner size={16} className="shrink-0" />
      ) : (
        <SparklesIcon className="size-4" />
      )}
    </button>
  );
}

function HeaderTabTranscript({
  isActive,
  onClick = () => {},
}: {
  isActive: boolean;
  onClick?: () => void;
}) {
  return (
    <IconHeaderTab
      isActive={isActive}
      label="Transcript"
      icon={<AudioLinesIcon className="size-4" />}
      onClick={onClick}
    />
  );
}

function useOpenTemplatesTab() {
  const openNew = useTabs((state) => state.openNew);
  const selectTab = useTabs((state) => state.select);
  const updateTemplatesTabState = useTabs(
    (state) => state.updateTemplatesTabState,
  );

  return useCallback(
    (state: Extract<Tab, { type: "templates" }>["state"]) => {
      const existingTemplatesTab = useTabs
        .getState()
        .tabs.find(
          (tab): tab is Extract<Tab, { type: "templates" }> =>
            tab.type === "templates",
        );

      if (!existingTemplatesTab) {
        openNew({ type: "templates", state });
        return;
      }

      updateTemplatesTabState(existingTemplatesTab, state);
      selectTab(existingTemplatesTab);
    },
    [openNew, selectTab, updateTemplatesTabState],
  );
}

function TemplatePickerPopover({
  sessionId,
  onSelectTemplate,
  trigger,
}: {
  sessionId: string;
  onSelectTemplate: (selection: TemplateSelection) => void;
  trigger: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const resultRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const sessionTitle = main.UI.useCell(
    "sessions",
    sessionId,
    "title",
    main.STORE_ID,
  ) as string | undefined;
  const rawMd = main.UI.useCell(
    "sessions",
    sessionId,
    "raw_md",
    main.STORE_ID,
  ) as string | undefined;
  const { data: transcriptSegments } = useTranscriptExportSegments(sessionId);
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
  const creatorName = useTemplateCreatorName();
  const {
    data: rawSuggestedTemplates = [],
    isLoading: isSuggestedTemplatesLoading,
  } = useWebResources<Record<string, unknown>>("templates");
  const suggestedTemplates = useMemo(
    () =>
      filterWebTemplatesAgainstUserTemplates({
        userTemplates,
        webTemplates: parseWebTemplates(rawSuggestedTemplates),
      }),
    [rawSuggestedTemplates, userTemplates],
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

  const handleSuggestedTemplateClick = useCallback(
    async (template: WebTemplate) => {
      const templateId = await createTemplate({
        title: template.title,
        description: template.description,
        category: template.category,
        targets: template.targets,
        sections: template.sections ?? [],
      });
      if (!templateId) {
        return;
      }

      handleUseTemplate({
        templateId,
        title: template.title || "Untitled",
      });
    },
    [createTemplate, handleUseTemplate],
  );

  const handleCreateTemplate = useCallback(
    async (title?: string) => {
      const nextTitle = title?.trim() || "New Template";

      const templateId = await createTemplate({
        title: nextTitle,
        description: "",
        sections: [],
      });
      if (!templateId) {
        return;
      }

      setOpen(false);
      setSearch("");
      resultRefs.current = [];
      openTemplatesTab({
        selectedMineId: templateId,
        selectedWebIndex: null,
        isWebMode: false,
        showHomepage: false,
      });
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
  const transcriptText = useMemo(
    () => formatTranscriptExportSegments(transcriptSegments),
    [transcriptSegments],
  );
  const meetingContent = useMemo(
    () =>
      [sessionTitle ?? "", extractPlainText(rawMd), transcriptText]
        .filter((value) => value.trim().length > 0)
        .join("\n\n"),
    [rawMd, sessionTitle, transcriptText],
  );
  const suggestedTemplateRecommendations = useMemo(
    () => rankSuggestedTemplates(suggestedTemplates, meetingContent),
    [meetingContent, suggestedTemplates],
  );

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

  const filteredSuggestedTemplates = useMemo(() => {
    if (!searchQuery) {
      return suggestedTemplateRecommendations;
    }

    return suggestedTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(searchQuery) ||
        template.description?.toLowerCase().includes(searchQuery) ||
        template.category?.toLowerCase().includes(searchQuery) ||
        template.targets?.some((target) =>
          target.toLowerCase().includes(searchQuery),
        ),
    );
  }, [searchQuery, suggestedTemplateRecommendations, suggestedTemplates]);

  const hasSearch = searchQuery.length > 0;
  const filteredWebTemplates = useMemo(() => {
    if (!searchQuery) {
      return suggestedTemplates;
    }

    return suggestedTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(searchQuery) ||
        template.description?.toLowerCase().includes(searchQuery) ||
        template.category?.toLowerCase().includes(searchQuery) ||
        template.targets?.some((target) =>
          target.toLowerCase().includes(searchQuery),
        ),
    );
  }, [searchQuery, suggestedTemplates]);
  const libraryTemplates = useMemo<
    Array<{
      key: string;
      title: string;
      description?: string;
      creatorLabel: string;
      tags?: string[];
      onClick: () => void;
    }>
  >(() => {
    const userItems = filteredOtherTemplates.map((template) => ({
      key: template.id,
      title: template.title || "Untitled",
      description: template.description,
      creatorLabel: getTemplateCreatorLabel({
        isUserTemplate: true,
        creatorName,
      }),
      tags: getTemplateTags(template),
      onClick: () =>
        handleUseTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
    }));

    const suggestedSlugs = new Set(
      !hasSearch
        ? filteredSuggestedTemplates.map(
            (template, index) => template.slug || `suggested-${index}`,
          )
        : [],
    );

    const webItems = filteredWebTemplates
      .filter((template, index) => {
        if (hasSearch) {
          return true;
        }

        return !suggestedSlugs.has(template.slug || `suggested-${index}`);
      })
      .map((template, index) => ({
        key: template.slug || `library-${index}`,
        title: template.title || "Untitled",
        description: template.description,
        creatorLabel: getTemplateCreatorLabel({ isUserTemplate: false }),
        tags: getTemplateTags(template),
        onClick: () => handleSuggestedTemplateClick(template),
      }));

    return [...userItems, ...webItems].sort((a, b) =>
      a.title.localeCompare(b.title),
    );
  }, [
    creatorName,
    filteredOtherTemplates,
    filteredSuggestedTemplates,
    filteredWebTemplates,
    handleSuggestedTemplateClick,
    handleUseTemplate,
    hasSearch,
  ]);
  const resultSections = useMemo<
    Array<{
      key: string;
      title: string;
      icon?: React.ReactNode;
      uppercase?: boolean;
      emptyMessage?: string;
      items: Array<{
        key: string;
        title: string;
        description?: string;
        creatorLabel?: string;
        tags?: string[];
        onClick: () => void;
      }>;
    }>
  >(() => {
    if (!hasSearch) {
      return [
        {
          key: "favorite",
          title: "Favorites",
          items: filteredFavoriteTemplates.map((template) => ({
            key: template.id,
            title: template.title || "Untitled",
            description: template.description,
            creatorLabel: getTemplateCreatorLabel({
              isUserTemplate: true,
              creatorName,
            }),
            tags: getTemplateTags(template),
            onClick: () =>
              handleUseTemplate({
                templateId: template.id,
                title: template.title || "Untitled",
              }),
          })),
          emptyMessage: "No favorite templates yet",
        },
        {
          key: "suggested",
          title: "Suggested",
          items: filteredSuggestedTemplates.map((template, index) => ({
            key: template.slug || `suggested-${index}`,
            title: template.title || "Untitled",
            description: template.description,
            creatorLabel: getTemplateCreatorLabel({ isUserTemplate: false }),
            tags: getTemplateTags(template),
            onClick: () => handleSuggestedTemplateClick(template),
          })),
          emptyMessage: isSuggestedTemplatesLoading
            ? "Loading suggestions..."
            : "No suggested templates yet",
        },
        ...(libraryTemplates.length > 0
          ? [
              {
                key: "library",
                title: "Templates",
                items: libraryTemplates,
              },
            ]
          : []),
      ];
    }

    return [
      {
        key: "create",
        title: "Create new template",
        icon: <PlusIcon className="h-3.5 w-3.5 text-blue-500" />,
        uppercase: false,
        items: [
          {
            key: `create-${trimmedSearch}`,
            title: trimmedSearch,
            onClick: () => handleCreateTemplate(trimmedSearch),
          },
        ],
      },
      ...(filteredFavoriteTemplates.length > 0
        ? [
            {
              key: "favorite",
              title: "Favorites",
              items: filteredFavoriteTemplates.map((template) => ({
                key: template.id,
                title: template.title || "Untitled",
                description: template.description,
                creatorLabel: getTemplateCreatorLabel({
                  isUserTemplate: true,
                  creatorName,
                }),
                tags: getTemplateTags(template),
                onClick: () =>
                  handleUseTemplate({
                    templateId: template.id,
                    title: template.title || "Untitled",
                  }),
              })),
            },
          ]
        : []),
      ...(libraryTemplates.length > 0
        ? [
            {
              key: "library",
              title: "Templates",
              items: libraryTemplates,
            },
          ]
        : []),
    ];
  }, [
    creatorName,
    filteredFavoriteTemplates,
    filteredSuggestedTemplates,
    handleCreateTemplate,
    handleSuggestedTemplateClick,
    handleUseTemplate,
    hasSearch,
    isSuggestedTemplatesLoading,
    libraryTemplates,
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
            <div className="border-border border-b py-2">
              <div
                className={cn(["flex h-9 items-center gap-2 rounded-md px-3"])}
              >
                <SearchIcon className="text-muted-foreground h-4 w-4" />
                <input
                  ref={searchInputRef}
                  autoFocus
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  onKeyDown={handleSearchInputKeyDown}
                  placeholder="Search templates..."
                  className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm focus:outline-hidden"
                />
                {search && (
                  <button
                    onClick={() => setSearch("")}
                    className="hover:bg-accent rounded-xs p-0.5"
                  >
                    <XIcon className="text-muted-foreground h-3 w-3" />
                  </button>
                )}
              </div>
            </div>

            <div className="relative">
              <div
                className={cn([
                  "scroll-fade-y scrollbar-hide max-h-80 overflow-y-auto p-2",
                ])}
              >
                <div className="flex flex-col gap-3">
                  {resultSections.map((section) => (
                    <TemplateSection
                      key={section.key}
                      title={section.title}
                      icon={section.icon}
                      uppercase={section.uppercase}
                    >
                      {section.items.length > 0 ? (
                        section.items.map((item) => {
                          const itemIndex = resultIndex;
                          resultIndex += 1;

                          return (
                            <TemplateResultButton
                              key={item.key}
                              buttonRef={(node) => {
                                resultRefs.current[itemIndex] = node;
                              }}
                              title={item.title}
                              description={item.description}
                              creatorLabel={item.creatorLabel}
                              tags={item.tags}
                              onClick={item.onClick}
                              onKeyDown={(e) =>
                                handleResultKeyDown(e, itemIndex)
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
            See all templates
            <ChevronRightIcon className="h-3.5 w-3.5" />
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

export function Header({
  sessionId,
  editorTabs,
  currentTab,
  handleTabChange,
}: {
  sessionId: string;
  editorTabs: EditorView[];
  currentTab: EditorView;
  handleTabChange: (view: EditorView) => void;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const primaryEnhancedTabId = editorTabs.find(
    (view): view is Extract<EditorView, { type: "enhanced" }> =>
      view.type === "enhanced",
  )?.id;

  return (
    <div data-tauri-drag-region className="flex flex-col">
      <div
        data-tauri-drag-region
        className="flex items-center justify-between gap-2"
      >
        <div data-tauri-drag-region className="relative min-w-0 flex-1">
          <div
            role="tablist"
            aria-label="Session note tabs"
            data-tauri-drag-region="false"
            className="bg-muted/25 pointer-events-auto relative z-10 flex h-7 w-fit max-w-full items-center gap-0.5 overflow-visible rounded-full"
          >
            {editorTabs.map((view, index) => {
              if (view.type === "enhanced") {
                return (
                  <HeaderTabEnhanced
                    key={`enhanced-${view.id}`}
                    sessionId={sessionId}
                    enhancedNoteId={view.id}
                    canRemove={view.id !== primaryEnhancedTabId}
                    onRemove={
                      view.id !== primaryEnhancedTabId
                        ? () => {
                            const previousView = editorTabs[index - 1];
                            if (
                              currentTab.type === "enhanced" &&
                              currentTab.id === view.id &&
                              previousView
                            ) {
                              handleTabChange(previousView);
                            }

                            store?.delRow("enhanced_notes", view.id);
                          }
                        : undefined
                    }
                    onSelectNote={(enhancedNoteId) =>
                      handleTabChange({ type: "enhanced", id: enhancedNoteId })
                    }
                    isActive={
                      currentTab.type === "enhanced" &&
                      currentTab.id === view.id
                    }
                    onClick={() => handleTabChange(view)}
                  />
                );
              }

              if (view.type === "raw") {
                return (
                  <HeaderTabRaw
                    key={view.type}
                    sessionId={sessionId}
                    isActive={currentTab.type === view.type}
                    onClick={() => handleTabChange(view)}
                  />
                );
              }

              if (view.type === "transcript") {
                return (
                  <HeaderTabTranscript
                    key={view.type}
                    isActive={currentTab.type === view.type}
                    onClick={() => handleTabChange(view)}
                  />
                );
              }

              return null;
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

export function useEditorTabs({
  sessionId,
}: {
  sessionId: string;
}): EditorView[] {
  useEnsureDefaultSummary(sessionId);
  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener((state) => state.batch[sessionId]?.error);
  const canShowTranscript =
    hasTranscript ||
    sessionMode === "active" ||
    sessionMode === "finalizing" ||
    sessionMode === "running_batch" ||
    Boolean(batchError);

  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );

  return createEditorTabs({
    enhancedNoteIds: enhancedNoteIds || [],
    canShowTranscript,
  });
}

function createEditorTabs({
  enhancedNoteIds,
  canShowTranscript,
}: {
  enhancedNoteIds: string[];
  canShowTranscript: boolean;
}): EditorView[] {
  const enhancedTabs: EditorView[] = enhancedNoteIds.map((id) => ({
    type: "enhanced",
    id,
  }));

  return [
    ...enhancedTabs,
    { type: "raw" },
    ...(canShowTranscript ? [{ type: "transcript" } as const] : []),
  ];
}

function useEnhanceLogic(sessionId: string, enhancedNoteId: string) {
  const model = useLanguageModel("enhance");
  const llmStatus = useLLMConnectionStatus();
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const [missingModelError, setMissingModelError] = useState<Error | null>(
    null,
  );

  const noteTemplateId =
    (main.UI.useCell(
      "enhanced_notes",
      enhancedNoteId,
      "template_id",
      main.STORE_ID,
    ) as string | undefined) || undefined;

  const enhanceTask = useAITaskTask(taskId, "enhance");

  const onRegenerate = useCallback(
    async (templateId: string | null) => {
      if (!model) {
        setMissingModelError(
          new Error("Intelligence provider not configured."),
        );
        return;
      }

      setMissingModelError(null);

      void analyticsCommands.event({
        event: "note_enhanced",
        is_auto: false,
      });

      await enhanceTask.start({
        model,
        args: {
          sessionId,
          enhancedNoteId,
          templateId: templateId ?? noteTemplateId,
        },
      });
    },
    [model, enhanceTask.start, sessionId, enhancedNoteId, noteTemplateId],
  );

  useEffect(() => {
    if (model && missingModelError) {
      setMissingModelError(null);
    }
  }, [model, missingModelError]);

  const isConfigError = shouldShowEmptySummaryConfigError(llmStatus);

  const isIdleWithConfigError = enhanceTask.isIdle && isConfigError;

  const error = missingModelError ?? enhanceTask.error;
  const isError =
    !!missingModelError || enhanceTask.isError || isIdleWithConfigError;

  return {
    isGenerating: enhanceTask.isGenerating,
    isError,
    error,
    onRegenerate,
    onCancel: enhanceTask.cancel,
  };
}

function getTemplateTags(template: { category?: string; targets?: string[] }) {
  return [
    ...new Set([
      ...(template.category ? [template.category] : []),
      ...(template.targets ?? []),
    ]),
  ];
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

const TEMPLATE_SUGGESTION_STOP_WORDS = new Set([
  "about",
  "after",
  "agenda",
  "also",
  "and",
  "before",
  "between",
  "call",
  "customer",
  "discussion",
  "discussions",
  "follow",
  "for",
  "from",
  "have",
  "into",
  "meeting",
  "meetings",
  "notes",
  "plan",
  "review",
  "session",
  "sessions",
  "template",
  "templates",
  "that",
  "their",
  "them",
  "this",
  "with",
  "your",
]);

function rankSuggestedTemplates(
  templates: WebTemplate[],
  meetingContent: string,
) {
  if (templates.length <= 3) {
    return templates;
  }

  const normalizedContent = normalizeTemplateSuggestionText(meetingContent);
  if (!normalizedContent) {
    return templates.slice(0, 3);
  }

  const rankedTemplates = templates
    .map((template, index) => ({
      template,
      index,
      score: getSuggestedTemplateScore(template, normalizedContent),
    }))
    .sort((a, b) => {
      if (b.score !== a.score) {
        return b.score - a.score;
      }
      return a.index - b.index;
    });

  if (rankedTemplates[0]?.score === 0) {
    return templates.slice(0, 3);
  }

  return rankedTemplates.slice(0, 3).map(({ template }) => template);
}

function getSuggestedTemplateScore(
  template: WebTemplate,
  normalizedContent: string,
) {
  let score = 0;

  const title = normalizeTemplateSuggestionText(template.title);
  const category = normalizeTemplateSuggestionText(template.category);

  if (title && normalizedContent.includes(title)) {
    score += 12;
  }

  if (category && normalizedContent.includes(category)) {
    score += 6;
  }

  template.targets?.forEach((target) => {
    const normalizedTarget = normalizeTemplateSuggestionText(target);
    if (normalizedTarget && normalizedContent.includes(normalizedTarget)) {
      score += 4;
    }
  });

  score += getTemplateSuggestionTokenMatches(
    normalizedContent,
    template.title,
    3,
  );
  score += getTemplateSuggestionTokenMatches(
    normalizedContent,
    template.category,
    2,
  );
  score += getTemplateSuggestionTokenMatches(
    normalizedContent,
    template.description,
    1,
  );

  template.targets?.forEach((target) => {
    score += getTemplateSuggestionTokenMatches(normalizedContent, target, 2);
  });

  return score;
}

function getTemplateSuggestionTokenMatches(
  normalizedContent: string,
  value: string | undefined,
  weight: number,
) {
  return tokenizeTemplateSuggestionText(value).reduce((score, token) => {
    if (normalizedContent.includes(token)) {
      return score + weight;
    }
    return score;
  }, 0);
}

function tokenizeTemplateSuggestionText(value: string | undefined) {
  return Array.from(
    new Set(
      normalizeTemplateSuggestionText(value)
        .split(/\s+/)
        .filter(
          (token) =>
            token.length > 2 && !TEMPLATE_SUGGESTION_STOP_WORDS.has(token),
        ),
    ),
  );
}

function normalizeTemplateSuggestionText(value: string | undefined) {
  return (value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9\s]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function TemplateSection({
  title,
  children,
  icon,
  uppercase = true,
}: {
  title: string;
  children: React.ReactNode;
  icon?: React.ReactNode;
  uppercase?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2 px-2">
        {icon ??
          (title === "Suggested templates" ? (
            <LightbulbIcon className="h-3.5 w-3.5 text-amber-500" />
          ) : title === "Favorite templates" ? (
            <HeartIcon className="h-3.5 w-3.5 text-rose-500" />
          ) : null)}
        <p
          className={cn([
            "text-muted-foreground font-mono text-[11px] font-medium tracking-wide",
            uppercase && "uppercase",
          ])}
        >
          {title}
        </p>
      </div>
      <div className="flex flex-col gap-1">{children}</div>
    </div>
  );
}

function TemplateResultButton({
  buttonRef,
  title,
  description,
  creatorLabel,
  tags,
  onClick,
  onKeyDown,
}: {
  buttonRef?: React.Ref<HTMLButtonElement>;
  title: string;
  description?: string;
  creatorLabel?: string;
  tags?: string[];
  onClick: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLButtonElement>) => void;
}) {
  return (
    <button
      ref={buttonRef}
      className={cn([
        "hover:bg-accent focus:bg-muted w-full rounded-md px-3 py-2 text-left transition-colors focus:outline-hidden",
        "flex flex-col gap-0.5",
      ])}
      onClick={onClick}
      onKeyDown={onKeyDown}
    >
      <span className="text-foreground truncate text-sm font-medium">
        {title}
      </span>
      {description ? (
        <span className="text-muted-foreground line-clamp-2 text-xs">
          {description}
        </span>
      ) : null}
      {creatorLabel ? (
        <span className="text-muted-foreground text-[11px]">
          {creatorLabel}
        </span>
      ) : null}
      {tags && tags.length > 0 ? (
        <span className="mt-1 flex flex-wrap gap-1">
          {tags.map((tag, index) => (
            <span
              key={`${tag}-${index}`}
              className="bg-muted text-muted-foreground rounded-xs px-1.5 py-0.5 text-[11px]"
            >
              {tag}
            </span>
          ))}
        </span>
      ) : null}
    </button>
  );
}
