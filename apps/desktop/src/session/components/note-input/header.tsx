import { useLingui } from "@lingui/react/macro";
import {
  AlignLeftIcon,
  AudioLinesIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  HeartIcon,
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

import { useAITaskTask } from "~/ai/hooks";
import { useLanguageModel, useLLMConnectionStatus } from "~/ai/hooks";
import * as AudioPlayer from "~/audio-player";
import { getEnhancerService } from "~/services/enhancer";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  buildTranscriptExportSegments,
  formatTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { useSessionTranscriptRenderData } from "~/session/components/note-input/transcript/render-request-hooks";
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
  parseWebTemplates,
  useCreateTemplate,
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
  size = "tray",
  className,
}: {
  isActive: boolean;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  title?: string;
  size?: "tray" | "standalone";
  className?: string;
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
        size,
        cn(["min-w-10 px-2", isActive ? "max-w-40 gap-1.5" : null, className]),
      )}
    >
      {icon}
      {isActive && (
        <span className="min-w-0 truncate text-xs font-medium">{label}</span>
      )}
    </button>
  );
}

function iconHeaderTabClassName(
  isActive: boolean,
  size: "tray" | "standalone" = "tray",
  className?: string,
) {
  const heightClassName = size === "tray" ? "h-[26px]" : "h-7";

  return cn([
    "flex shrink-0 items-center justify-center rounded-full transition-colors select-none [&>svg]:shrink-0",
    isActive
      ? [
          "text-foreground bg-white shadow-xs",
          "dark:text-primary dark:bg-white",
        ]
      : [
          "text-muted-foreground/70",
          "hover:bg-background/60 hover:text-foreground",
          "dark:hover:bg-white/8",
        ],
    heightClassName,
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
  standalone = false,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  standalone?: boolean;
}) {
  const { t } = useLingui();
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
      label={t`Memos`}
      icon={<AlignLeftIcon className="size-4" />}
      onClick={onClick}
      onContextMenu={showContextMenu}
      size={standalone ? "standalone" : "tray"}
      className={standalone ? "border-border/70 border shadow-xs" : undefined}
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
        "tray",
        cn([
          "max-w-56 min-w-[62px] gap-1.5 pr-1.5 pl-2",
          isGenerating ? "cursor-not-allowed opacity-70" : "cursor-pointer",
          isError
            ? [
                "text-red-600 hover:bg-red-50 hover:text-red-700 focus-visible:bg-red-50",
                "dark:text-red-400 dark:hover:bg-red-950/50 dark:hover:text-red-300 dark:focus-visible:bg-red-950/50",
              ]
            : [
                "focus-visible:text-foreground focus-visible:bg-white",
                "dark:focus-visible:text-primary dark:focus-visible:bg-white",
              ],
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
      className={iconHeaderTabClassName(false, "tray", "min-w-10 px-2")}
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
  sessionId,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
}) {
  const { t } = useLingui();
  const regenerate = useRegenerateTranscript(sessionId);
  const { request: transcriptExportRequest } =
    useSessionTranscriptRenderData(sessionId);
  const { audioExists, deleteRecording, isDeletingRecording } =
    AudioPlayer.useAudioPlayer();
  const canCopyTranscript = Boolean(transcriptExportRequest);
  const handleCopyTranscript = useCallback(async () => {
    if (!transcriptExportRequest) {
      return;
    }

    try {
      const transcriptSegments = await buildTranscriptExportSegments(
        transcriptExportRequest,
      );
      const transcriptText = formatTranscriptExportSegments(transcriptSegments);
      if (!transcriptText) {
        return;
      }

      await copyTextToClipboard(transcriptText, {
        success: "Transcript copied to clipboard",
        error: "Failed to copy transcript",
      });
    } catch (error) {
      console.error("Failed to copy transcript", error);
      sonnerToast.error("Failed to copy transcript");
    }
  }, [transcriptExportRequest]);
  const handleDeleteRecording = useCallback(() => {
    void deleteRecording();
  }, [deleteRecording]);
  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const items: MenuItemDef[] = [
      {
        id: `copy-transcript-${sessionId}`,
        text: "Copy",
        action: () => {
          void handleCopyTranscript();
        },
        disabled: !canCopyTranscript,
      },
    ];

    if (audioExists) {
      items.push({
        id: `regenerate-transcript-${sessionId}`,
        text: "Regenerate",
        action: () => {
          void regenerate();
        },
      });
      items.push({
        id: `delete-recording-${sessionId}`,
        text: "Delete recording",
        action: handleDeleteRecording,
        disabled: isDeletingRecording,
      });
    }

    return items;
  }, [
    audioExists,
    canCopyTranscript,
    handleCopyTranscript,
    handleDeleteRecording,
    isDeletingRecording,
    regenerate,
    sessionId,
  ]);
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <IconHeaderTab
      isActive={isActive}
      label={t`Transcript`}
      icon={<AudioLinesIcon className="size-4" />}
      onClick={onClick}
      onContextMenu={showContextMenu}
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
  onSelectTemplate,
  trigger,
}: {
  onSelectTemplate: (selection: TemplateSelection) => void;
  trigger: React.ReactNode;
}) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const resultRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
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
      isFavorite?: boolean;
      onClick: () => void;
    }>
  >(() => {
    const favoriteItems = filteredFavoriteTemplates.map((template) => ({
      key: template.id,
      title: template.title || "Untitled",
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
      onClick: () =>
        handleUseTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
    }));

    const webItems = filteredWebTemplates.map((template, index) => ({
      key: template.slug || `library-${index}`,
      title: template.title || "Untitled",
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
        isFavorite?: boolean;
        onClick: () => void;
      }>;
    }>
  >(() => {
    if (!hasSearch) {
      return [
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
  }, [handleCreateTemplate, hasSearch, templateItems, trimmedSearch]);
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
                  placeholder={t`Search templates...`}
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
                className={cn(["scroll-fade-y max-h-80 overflow-y-auto p-2"])}
              >
                <div className="flex flex-col gap-3">
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

                          return (
                            <TemplateResultButton
                              key={item.key}
                              buttonRef={(node) => {
                                resultRefs.current[itemIndex] = node;
                              }}
                              title={item.title}
                              isFavorite={item.isFavorite}
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
            {t`See all templates`}
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
  const { t } = useLingui();
  const store = main.UI.useStore(main.STORE_ID);
  const primaryEnhancedTabId = editorTabs.find(
    (view): view is Extract<EditorView, { type: "enhanced" }> =>
      view.type === "enhanced",
  )?.id;
  const shouldUseTabTray = editorTabs.length > 1;

  return (
    <div data-tauri-drag-region className="flex flex-col pl-1">
      <div
        data-tauri-drag-region
        className="flex items-center justify-between gap-2"
      >
        <div data-tauri-drag-region className="relative min-w-0 flex-1">
          <div
            role="tablist"
            aria-label={t`Session note tabs`}
            data-tauri-drag-region="false"
            className={cn([
              "pointer-events-auto relative z-10 w-fit max-w-full overflow-visible",
              shouldUseTabTray
                ? "bg-foreground/10 flex h-[30px] items-center gap-[2px] rounded-full p-[2px] dark:bg-white/12"
                : null,
            ])}
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
                    standalone={!shouldUseTabTray}
                    onClick={() => handleTabChange(view)}
                  />
                );
              }

              if (view.type === "transcript") {
                return (
                  <HeaderTabTranscript
                    key={view.type}
                    sessionId={sessionId}
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
    <div className="flex flex-col gap-1">
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
      <div className="flex flex-col gap-1">{children}</div>
    </div>
  );
}

function TemplateResultButton({
  buttonRef,
  title,
  isFavorite = false,
  onClick,
  onKeyDown,
}: {
  buttonRef?: React.Ref<HTMLButtonElement>;
  title: string;
  isFavorite?: boolean;
  onClick: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLButtonElement>) => void;
}) {
  return (
    <button
      ref={buttonRef}
      className={cn([
        "hover:bg-accent focus:bg-muted w-full rounded-md px-3 py-2 text-left transition-colors focus:outline-hidden",
        "flex items-center gap-1.5",
      ])}
      onClick={onClick}
      onKeyDown={onKeyDown}
    >
      <span className="text-foreground min-w-0 truncate text-sm font-medium">
        {title}
      </span>
      {isFavorite ? (
        <HeartIcon
          aria-hidden
          className="size-3.5 shrink-0 fill-rose-500 text-rose-500"
        />
      ) : null}
    </button>
  );
}
