import { useQuery } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  CheckIcon,
  ChevronRightIcon,
  CopyIcon,
  HeartIcon,
  LightbulbIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  XIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import {
  type AttachmentInfo,
  commands as fsSyncCommands,
} from "@hypr/plugin-fs-sync";
import { NoteTab } from "@hypr/ui/components/ui/note-tab";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import {
  formatTranscriptExportSegments,
  useTranscriptExportSegments,
} from "./transcript/export-data";

import { useAITaskTask } from "~/ai/hooks";
import { useLanguageModel, useLLMConnectionStatus } from "~/ai/hooks";
import { useAudioPlayer } from "~/audio-player";
import { extractPlainText } from "~/search/contexts/engine/utils";
import { getEnhancerService } from "~/services/enhancer";
import { useHasTranscript } from "~/session/components/shared";
import { useEnsureDefaultSummary } from "~/session/hooks/useEnhancedNotes";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { type TaskStepInfo } from "~/store/zustand/ai-task/tasks";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useRunBatch } from "~/stt/useRunBatch";
import { useUserTemplates } from "~/templates";

function TruncatedTitle({
  title,
  isActive,
}: {
  title: string;
  isActive: boolean;
}) {
  return (
    <span
      className={cn(["truncate", isActive ? "max-w-[120px]" : "max-w-[60px]"])}
    >
      {title}
    </span>
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
  const { audioExists } = useAudioPlayer();
  const { sessionMode, progressRaw } = useListener((state) => ({
    sessionMode: state.getSessionMode(sessionId),
    progressRaw: state.batch[sessionId] ?? null,
  }));
  const batchError = progressRaw?.error ?? null;
  const isBatchProcessing = sessionMode === "running_batch";
  const isSessionInactive =
    sessionMode !== "active" &&
    sessionMode !== "finalizing" &&
    sessionMode !== "running_batch";
  const store = main.UI.useStore(main.STORE_ID);
  const runBatch = useRunBatch(sessionId);
  const [isRedoing, setIsRedoing] = useState(false);
  const [copied, setCopied] = useState(false);
  const copiedResetTimeoutRef = useRef<number | null>(null);

  const isProcessing = isBatchProcessing || isRedoing;
  const { data: transcriptSegments } = useTranscriptExportSegments(sessionId);
  const transcriptText = useMemo(
    () => formatTranscriptExportSegments(transcriptSegments),
    [transcriptSegments],
  );

  useEffect(() => {
    return () => {
      if (copiedResetTimeoutRef.current !== null) {
        window.clearTimeout(copiedResetTimeoutRef.current);
      }
    };
  }, []);

  const handleRefreshClick = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();

      if (!audioExists || isBatchProcessing || !store) {
        return;
      }

      setIsRedoing(true);

      const oldTranscriptIds: string[] = [];
      store.forEachRow("transcripts", (transcriptId, _forEachCell) => {
        const session = store.getCell(
          "transcripts",
          transcriptId,
          "session_id",
        );
        if (session === sessionId) {
          oldTranscriptIds.push(transcriptId);
        }
      });

      getEnhancerService()?.resetEnhanceTasks(sessionId);

      try {
        const result = await fsSyncCommands.audioPath(sessionId);
        if (result.status === "error") {
          throw new Error(result.error);
        }

        const audioPath = result.data;
        if (!audioPath) {
          throw new Error("audio path not available");
        }

        await runBatch(audioPath);

        if (oldTranscriptIds.length > 0) {
          store.transaction(() => {
            oldTranscriptIds.forEach((id) => {
              store.delRow("transcripts", id);
            });
          });
        }

        getEnhancerService()?.queueAutoEnhance(sessionId);
      } catch (error) {
        const message =
          error instanceof Error
            ? error.message
            : typeof error === "string"
              ? error
              : JSON.stringify(error);
        console.error("[redo_transcript] failed:", message);
      } finally {
        setIsRedoing(false);
      }
    },
    [audioExists, isBatchProcessing, runBatch, sessionId, store],
  );
  const handleCopyClick = useCallback(
    async (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (!transcriptText) {
        return;
      }

      try {
        await navigator.clipboard.writeText(transcriptText);
        if (copiedResetTimeoutRef.current !== null) {
          window.clearTimeout(copiedResetTimeoutRef.current);
        }
        setCopied(true);
        copiedResetTimeoutRef.current = window.setTimeout(() => {
          setCopied(false);
          copiedResetTimeoutRef.current = null;
        }, 2000);
      } catch {}
    },
    [transcriptText],
  );

  const showRefreshButton = audioExists && isActive && isSessionInactive;
  const showCopyButton =
    isActive && isSessionInactive && transcriptText.length > 0;
  const showProgress = audioExists && isActive && isProcessing;
  const refreshButton = (
    <span
      onClick={handleRefreshClick}
      className={cn([
        "inline-flex h-5 w-5 cursor-pointer items-center justify-center rounded-xs transition-colors",
        batchError
          ? [
              "text-red-600 hover:bg-red-50 focus-visible:bg-red-50",
              "hover:text-neutral-900 focus-visible:text-neutral-900",
            ]
          : ["hover:bg-neutral-200 focus-visible:bg-neutral-200"],
      ])}
    >
      <RefreshCwIcon size={12} />
    </span>
  );
  const copyButton = (
    <span
      onClick={(e) => {
        void handleCopyClick(e);
      }}
      className={cn([
        "inline-flex h-5 w-5 cursor-pointer items-center justify-center rounded-xs transition-colors",
        copied
          ? "text-green-500"
          : ["hover:bg-neutral-200 focus-visible:bg-neutral-200"],
      ])}
      aria-label="Copy transcript"
      role="button"
    >
      {copied ? <CheckIcon size={12} /> : <CopyIcon size={12} />}
    </span>
  );

  return (
    <NoteTab isActive={isActive} onClick={onClick}>
      Transcript
      {showCopyButton && (
        <Tooltip>
          <TooltipTrigger asChild>{copyButton}</TooltipTrigger>
          <TooltipContent>
            {copied ? "Copied" : "Copy transcript"}
          </TooltipContent>
        </Tooltip>
      )}
      {showRefreshButton &&
        (batchError ? (
          <Tooltip>
            <TooltipTrigger asChild>{refreshButton}</TooltipTrigger>
            <TooltipContent>{batchError}</TooltipContent>
          </Tooltip>
        ) : (
          refreshButton
        ))}
      {showProgress && (
        <span className="inline-flex items-center text-neutral-500">
          <Spinner size={12} />
        </span>
      )}
    </NoteTab>
  );
}

function HeaderTabEnhanced({
  isActive,
  onClick = () => {},
  sessionId,
  enhancedNoteId,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  enhancedNoteId: string;
}) {
  const { isGenerating, isError, onRegenerate, onCancel, currentStep } =
    useEnhanceLogic(sessionId, enhancedNoteId);

  const title =
    main.UI.useCell("enhanced_notes", enhancedNoteId, "title", main.STORE_ID) ||
    "Summary";

  const handleRegenerateClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      void onRegenerate(null);
    },
    [onRegenerate],
  );

  if (isGenerating) {
    const step = currentStep as TaskStepInfo<"enhance"> | undefined;

    const handleCancelClick = (e: React.MouseEvent) => {
      e.stopPropagation();
      onCancel();
    };

    return (
      <div
        role="button"
        tabIndex={0}
        onClick={onClick}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onClick();
          }
        }}
        className={cn([
          "group/tab relative my-2 shrink-0 cursor-pointer border-b-2 px-1 py-0.5 text-xs font-medium transition-all duration-200",
          isActive
            ? ["text-neutral-900", "border-neutral-900"]
            : [
                "text-neutral-600",
                "border-transparent",
                "hover:text-neutral-800",
              ],
        ])}
      >
        <span className="flex h-5 items-center gap-1">
          <TruncatedTitle title={title} isActive={isActive} />
          <button
            type="button"
            onClick={handleCancelClick}
            className={cn([
              "inline-flex h-5 w-5 cursor-pointer items-center justify-center rounded-xs hover:bg-neutral-200",
              !isActive && "opacity-50",
            ])}
            aria-label="Cancel enhancement"
          >
            <span className="flex items-center justify-center group-hover/tab:hidden">
              {step?.type === "generating" ? (
                <img
                  src="/assets/write-animation.gif"
                  alt=""
                  aria-hidden="true"
                  className="size-3"
                />
              ) : (
                <Spinner size={14} />
              )}
            </span>
            <XIcon className="hidden size-4 items-center justify-center group-hover/tab:flex" />
          </button>
        </span>
      </div>
    );
  }

  const regenerateIcon = (
    <span
      onClick={handleRegenerateClick}
      className={cn([
        "group relative inline-flex h-5 w-5 cursor-pointer items-center justify-center rounded-xs transition-colors",
        isError
          ? [
              "text-red-600 hover:bg-red-50 hover:text-neutral-900 focus-visible:bg-red-50 focus-visible:text-neutral-900",
            ]
          : ["hover:bg-neutral-200 focus-visible:bg-neutral-200"],
      ])}
    >
      {isError && (
        <AlertCircleIcon
          size={12}
          className="pointer-events-none absolute inset-0 m-auto transition-opacity duration-200 group-hover:opacity-0 group-focus-visible:opacity-0"
        />
      )}
      <RefreshCwIcon
        size={12}
        className={cn([
          "pointer-events-none absolute inset-0 m-auto transition-opacity duration-200",
          isError
            ? "opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100"
            : "opacity-100",
        ])}
      />
    </span>
  );

  return (
    <NoteTab isActive={isActive} onClick={onClick}>
      <TruncatedTitle title={title} isActive={isActive} />
      {isActive && regenerateIcon}
    </NoteTab>
  );
}

function CreateOtherFormatButton({
  sessionId,
  handleTabChange,
}: {
  sessionId: string;
  handleTabChange: (view: EditorView) => void;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const resultRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const { user_id } = main.UI.useValues(main.STORE_ID);
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
  const {
    data: suggestedTemplates = [],
    isLoading: isSuggestedTemplatesLoading,
  } = useWebResources<WebTemplate>("templates");
  const openNew = useTabs((state) => state.openNew);
  const selectTab = useTabs((state) => state.select);
  const updateTemplatesTabState = useTabs(
    (state) => state.updateTemplatesTabState,
  );
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

  const openTemplatesTab = useCallback(
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

  const handleUseTemplate = useCallback(
    (templateId: string) => {
      setOpen(false);
      setSearch("");
      resultRefs.current = [];

      const service = getEnhancerService();
      if (!service) return;

      const result = service.enhance(sessionId, { templateId });
      if (result.type === "started" || result.type === "already_active") {
        handleTabChange({ type: "enhanced", id: result.noteId });
      }
    },
    [sessionId, handleTabChange],
  );

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setSearch("");
      resultRefs.current = [];
    }
  }, []);

  const handleSuggestedTemplateClick = useCallback(
    (template: WebTemplate) => {
      if (!user_id) return;

      const templateId = crypto.randomUUID();
      const now = new Date().toISOString();

      setRow({
        id: templateId,
        user_id,
        created_at: now,
        title: template.title,
        description: template.description,
        sections: template.sections ?? [],
      });

      handleUseTemplate(templateId);
    },
    [handleUseTemplate, setRow, user_id],
  );

  const handleCreateTemplate = useCallback(
    (title?: string) => {
      if (!user_id) return;

      const templateId = crypto.randomUUID();
      const now = new Date().toISOString();
      const nextTitle = title?.trim() || "New Template";

      setRow({
        id: templateId,
        user_id,
        created_at: now,
        title: nextTitle,
        description: "",
        sections: [],
      });

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
    [openTemplatesTab, setRow, user_id],
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

  const filteredFavoriteTemplates = useMemo(() => {
    const sortedTemplates = [...userTemplates].sort((a, b) =>
      (a.title || "").localeCompare(b.title || ""),
    );

    if (!searchQuery) {
      return sortedTemplates;
    }

    return sortedTemplates.filter(
      (template) =>
        template.title?.toLowerCase().includes(searchQuery) ||
        template.description?.toLowerCase().includes(searchQuery),
    );
  }, [searchQuery, userTemplates]);

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
        onClick: () => void;
      }>;
    }>
  >(() => {
    if (!hasSearch) {
      return [
        {
          key: "suggested",
          title: "Suggested templates",
          items: filteredSuggestedTemplates.map((template, index) => ({
            key: template.slug || `suggested-${index}`,
            title: template.title || "Untitled",
            description: template.description,
            onClick: () => handleSuggestedTemplateClick(template),
          })),
          emptyMessage: isSuggestedTemplatesLoading
            ? "Loading suggestions..."
            : "No suggested templates yet",
        },
        {
          key: "favorite",
          title: "Favorite templates",
          items: filteredFavoriteTemplates.map((template) => ({
            key: template.id,
            title: template.title || "Untitled",
            description: template.description,
            onClick: () => handleUseTemplate(template.id),
          })),
          emptyMessage: "No favorite templates yet",
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
      ...(filteredSuggestedTemplates.length > 0
        ? [
            {
              key: "suggested",
              title: "Suggested templates",
              items: filteredSuggestedTemplates.map((template, index) => ({
                key: template.slug || `suggested-${index}`,
                title: template.title || "Untitled",
                description: template.description,
                onClick: () => handleSuggestedTemplateClick(template),
              })),
            },
          ]
        : []),
      ...(filteredFavoriteTemplates.length > 0
        ? [
            {
              key: "favorite",
              title: "Favorite templates",
              items: filteredFavoriteTemplates.map((template) => ({
                key: template.id,
                title: template.title || "Untitled",
                description: template.description,
                onClick: () => handleUseTemplate(template.id),
              })),
            },
          ]
        : []),
    ];
  }, [
    filteredFavoriteTemplates,
    filteredSuggestedTemplates,
    handleCreateTemplate,
    handleSuggestedTemplateClick,
    handleUseTemplate,
    hasSearch,
    isSuggestedTemplatesLoading,
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
      <PopoverTrigger asChild>
        <button
          className={cn([
            "relative my-2 shrink-0 px-1 py-0.5 text-xs font-medium whitespace-nowrap transition-all duration-200",
            "text-neutral-600 hover:text-neutral-800",
            "flex items-center gap-1",
            "border-b-2 border-transparent",
          ])}
        >
          <PlusIcon size={14} />
          <span>Use template</span>
        </button>
      </PopoverTrigger>
      <PopoverContent variant="app" className="w-80" align="start">
        <div className="flex flex-col gap-1">
          <AppFloatingPanel className="flex flex-col overflow-hidden">
            <div className="border-b border-neutral-200 py-2">
              <div
                className={cn([
                  "flex h-9 items-center gap-2 rounded-md bg-white px-3",
                ])}
              >
                <SearchIcon className="h-4 w-4 text-neutral-400" />
                <input
                  ref={searchInputRef}
                  autoFocus
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  onKeyDown={handleSearchInputKeyDown}
                  placeholder="Search templates..."
                  className="flex-1 bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
                />
                {search && (
                  <button
                    onClick={() => setSearch("")}
                    className="rounded-xs p-0.5 hover:bg-neutral-100"
                  >
                    <XIcon className="h-3 w-3 text-neutral-400" />
                  </button>
                )}
              </div>
            </div>

            <div className="max-h-80 overflow-y-auto p-2">
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
                            onClick={item.onClick}
                            onKeyDown={(e) => handleResultKeyDown(e, itemIndex)}
                          />
                        );
                      })
                    ) : (
                      <div className="px-2 py-3 text-sm text-neutral-500">
                        {section.emptyMessage}
                      </div>
                    )}
                  </TemplateSection>
                ))}
              </div>
            </div>
          </AppFloatingPanel>

          <button
            onClick={handleSeeAllTemplates}
            className={cn([
              "flex h-7 w-full items-center justify-center gap-1 rounded-lg px-3 text-xs font-medium",
              "text-neutral-600 transition-colors hover:bg-neutral-100 hover:text-neutral-900",
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
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const isLiveProcessing = sessionMode === "active";

  const tabsRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(tabsRef, "horizontal", [
    editorTabs.length,
  ]);

  if (editorTabs.length === 1 && editorTabs[0].type === "raw") {
    return null;
  }

  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between gap-2">
        <div className="relative min-w-0 flex-1">
          <div
            ref={tabsRef}
            className="scrollbar-hide flex items-center gap-1 overflow-x-auto"
          >
            {editorTabs.map((view) => {
              if (view.type === "enhanced") {
                return (
                  <HeaderTabEnhanced
                    key={`enhanced-${view.id}`}
                    sessionId={sessionId}
                    enhancedNoteId={view.id}
                    isActive={
                      currentTab.type === "enhanced" &&
                      currentTab.id === view.id
                    }
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

              return (
                <NoteTab
                  key={view.type}
                  isActive={currentTab.type === view.type}
                  onClick={() => handleTabChange(view)}
                >
                  {labelForEditorView(view)}
                </NoteTab>
              );
            })}
            {!isLiveProcessing && (
              <CreateOtherFormatButton
                sessionId={sessionId}
                handleTabChange={handleTabChange}
              />
            )}
          </div>
          {!atStart && <ScrollFadeOverlay position="left" />}
          {!atEnd && <ScrollFadeOverlay position="right" />}
        </div>
      </div>
    </div>
  );
}

export function useAttachments(sessionId: string): {
  attachments: AttachmentInfo[];
  isLoading: boolean;
  refetch: () => void;
} {
  const { data, isLoading, refetch } = useQuery({
    queryKey: ["attachments", sessionId],
    queryFn: async () => {
      const result = await fsSyncCommands.attachmentList(sessionId);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  return {
    attachments: data ?? [],
    isLoading,
    refetch,
  };
}

export function useEditorTabs({
  sessionId,
}: {
  sessionId: string;
}): EditorView[] {
  useEnsureDefaultSummary(sessionId);

  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const hasTranscript = useHasTranscript(sessionId);
  const { attachments } = useAttachments(sessionId);
  const hasAttachments = attachments.length > 0;
  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );

  if (sessionMode === "active") {
    const tabs: EditorView[] = [{ type: "raw" }, { type: "transcript" }];
    if (hasAttachments) {
      tabs.push({ type: "attachments" });
    }
    return tabs;
  }

  if (hasTranscript) {
    const enhancedTabs: EditorView[] = (enhancedNoteIds || []).map((id) => ({
      type: "enhanced",
      id,
    }));
    const tabs: EditorView[] = [
      ...enhancedTabs,
      { type: "raw" },
      { type: "transcript" },
    ];
    if (hasAttachments) {
      tabs.push({ type: "attachments" });
    }
    return tabs;
  }

  const tabs: EditorView[] = [{ type: "raw" }];
  if (hasAttachments) {
    tabs.push({ type: "attachments" });
  }
  return tabs;
}

function labelForEditorView(view: EditorView): string {
  if (view.type === "enhanced") {
    return "Summary";
  }
  if (view.type === "raw") {
    return "Memos";
  }
  if (view.type === "transcript") {
    return "Transcript";
  }
  if (view.type === "attachments") {
    return "Attachments";
  }
  return "";
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

  const isConfigError =
    llmStatus.status === "pending" ||
    (llmStatus.status === "error" &&
      (llmStatus.reason === "missing_config" ||
        llmStatus.reason === "not_pro" ||
        llmStatus.reason === "unauthenticated"));

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
    currentStep: enhanceTask.currentStep,
  };
}

type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  sections: Array<{ title: string; description: string }>;
};

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
            "font-mono text-[11px] font-medium tracking-wide text-neutral-500",
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
  onClick,
  onKeyDown,
}: {
  buttonRef?: React.Ref<HTMLButtonElement>;
  title: string;
  description?: string;
  onClick: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLButtonElement>) => void;
}) {
  return (
    <button
      ref={buttonRef}
      className={cn([
        "w-full rounded-md px-3 py-2 text-left transition-colors hover:bg-neutral-100 focus:bg-neutral-100 focus:outline-hidden",
        "flex flex-col gap-0.5",
      ])}
      onClick={onClick}
      onKeyDown={onKeyDown}
    >
      <span className="truncate text-sm font-medium text-neutral-900">
        {title}
      </span>
      {description ? (
        <span className="line-clamp-2 text-xs text-neutral-500">
          {description}
        </span>
      ) : null}
    </button>
  );
}
