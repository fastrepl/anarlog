import { useLingui } from "@lingui/react/macro";
import {
  AlignLeftIcon,
  AudioLinesIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  HeartIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  SparklesIcon,
  SquareIcon,
  XIcon,
} from "lucide-react";
import { LightbulbIcon } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

import { json2md, parseJsonContent } from "@hypr/editor/markdown";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import * as AudioPlayer from "~/audio-player";
import { getEnhancerService } from "~/services/enhancer";
import { useEnhancedNoteActions } from "~/session/components/note-input/enhanced-actions";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  buildTranscriptExportSegments,
  formatTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { useSessionTranscriptRenderData } from "~/session/components/note-input/transcript/render-request-hooks";
import { useCanShowTranscript } from "~/session/components/shared";
import { useEnsureDefaultSummary } from "~/session/hooks/useEnhancedNotes";
import { useCanShowInsights } from "~/session/insights/past-notes";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
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
          "dark:bg-accent dark:text-foreground dark:shadow-none",
        ]
      : [
          "text-muted-foreground/70",
          "hover:bg-background/60 hover:text-foreground",
          "dark:hover:bg-accent/80 dark:hover:text-foreground",
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
  templateId: string | null;
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
  if (!isActive) {
    return (
      <HeaderTabRawButton
        isActive={isActive}
        onClick={onClick}
        standalone={standalone}
      />
    );
  }

  return (
    <HeaderTabRawActive
      isActive={isActive}
      onClick={onClick}
      sessionId={sessionId}
      standalone={standalone}
    />
  );
}

function HeaderTabRawButton({
  isActive,
  onClick,
  onContextMenu,
  standalone,
}: {
  isActive: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  standalone: boolean;
}) {
  const { t } = useLingui();

  return (
    <IconHeaderTab
      isActive={isActive}
      label={t`Memos`}
      icon={<AlignLeftIcon className="size-4" />}
      onClick={onClick}
      onContextMenu={onContextMenu}
      size={standalone ? "standalone" : "tray"}
      className={standalone ? "border-border/70 border shadow-xs" : undefined}
    />
  );
}

function HeaderTabRawActive({
  isActive,
  onClick,
  sessionId,
  standalone,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  standalone: boolean;
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
    <HeaderTabRawButton
      isActive={isActive}
      onClick={onClick}
      onContextMenu={showContextMenu}
      standalone={standalone}
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
  if (!isActive) {
    return (
      <HeaderTabEnhancedInactive
        sessionId={sessionId}
        enhancedNoteId={enhancedNoteId}
        onClick={onClick}
      />
    );
  }

  return (
    <HeaderTabEnhancedActive
      sessionId={sessionId}
      enhancedNoteId={enhancedNoteId}
      canRemove={canRemove}
      onRemove={onRemove}
      onSelectNote={onSelectNote}
    />
  );
}

function useEnhancedTabTitle(enhancedNoteId: string) {
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

  return {
    tabTitle,
    templateId: templateId ?? null,
    templateTooltip:
      templateId && templateTitle
        ? `${templateTitle} was used to generate this summary.`
        : undefined,
  };
}

function HeaderTabEnhancedInactive({
  onClick = () => {},
  sessionId,
  enhancedNoteId,
}: {
  sessionId: string;
  enhancedNoteId: string;
  onClick?: () => void;
}) {
  const { tabTitle, templateTooltip } = useEnhancedTabTitle(enhancedNoteId);
  const { isGenerating } = useEnhanceLogic(sessionId, enhancedNoteId);

  return (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={tabTitle}
      onClick={onClick}
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

function HeaderTabEnhancedActive({
  sessionId,
  enhancedNoteId,
  canRemove = false,
  onRemove,
  onSelectNote,
}: {
  sessionId: string;
  enhancedNoteId: string;
  canRemove?: boolean;
  onRemove?: () => void;
  onSelectNote?: (enhancedNoteId: string) => void;
}) {
  const { t } = useLingui();
  const { isGenerating, isError, onCancel, onRegenerate } = useEnhanceLogic(
    sessionId,
    enhancedNoteId,
  );
  const content = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "content",
    main.STORE_ID,
  ) as string | undefined;
  const { tabTitle, templateId, templateTooltip } =
    useEnhancedTabTitle(enhancedNoteId);
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
  const handleTabClick = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      event.stopPropagation();

      if (isGenerating) {
        event.preventDefault();
        onCancel();
      }
    },
    [isGenerating, onCancel],
  );
  const applyTemplate = useCallback(
    (selection: TemplateSelection) => {
      if (isGenerating) {
        return;
      }

      const result = getEnhancerService()?.enhance(sessionId, {
        templateId: selection.templateId,
        targetNoteId: enhancedNoteId,
        templateTitle: selection.templateId ? selection.title : undefined,
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
  const handleSelectTemplate = useCallback(
    (selection: TemplateSelection) => {
      applyTemplate(selection);
    },
    [applyTemplate],
  );
  const handleRegenerateTemplate = useCallback(
    (selection: TemplateSelection) => {
      applyTemplate(selection);
    },
    [applyTemplate],
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
  const templateMenuTrigger = (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={tabTitle}
      aria-current="page"
      onClick={handleTabClick}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={showContextMenu}
      title={isGenerating ? "Stop summary" : templateTooltip}
      data-hover-label={isGenerating ? t`Stop` : undefined}
      className={iconHeaderTabClassName(
        true,
        "tray",
        cn([
          "group/summary-stop max-w-56 min-w-[62px] gap-1.5 pr-1.5 pl-2",
          isGenerating
            ? [
                "w-[112px] min-w-[112px] cursor-pointer opacity-70",
                "after:hidden after:min-w-0 after:truncate after:text-xs after:font-medium after:content-[attr(data-hover-label)] hover:after:block",
              ]
            : "cursor-pointer",
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
        <SummaryGeneratingIcon />
      ) : (
        <SparklesIcon className="size-4" />
      )}
      <span
        className={cn([
          "min-w-0 truncate text-xs font-medium",
          isGenerating ? "group-hover/summary-stop:hidden" : null,
        ])}
      >
        {tabTitle}
      </span>
      <ChevronDownIcon
        className={cn([
          "size-3.5",
          isGenerating ? "group-hover/summary-stop:hidden" : null,
        ])}
      />
    </button>
  );

  return (
    <TemplatePickerPopover
      selectedTemplateId={templateId}
      onSelectTemplate={handleSelectTemplate}
      onRegenerateTemplate={handleRegenerateTemplate}
      trigger={templateMenuTrigger}
    />
  );
}

function SummaryGeneratingIcon() {
  return (
    <span className="relative flex size-4 items-center justify-center">
      <Spinner size={16} className="shrink-0 group-hover/summary-stop:hidden" />
      <SquareIcon className="hidden size-3 fill-current group-hover/summary-stop:block" />
    </span>
  );
}

function HeaderTabTranscript({
  isActive,
  isTranscribing,
  canStopTranscription,
  onClick = () => {},
  sessionId,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  canStopTranscription: boolean;
  onClick?: () => void;
  sessionId: string;
}) {
  if (!isActive) {
    return (
      <HeaderTabTranscriptButton
        isActive={isActive}
        isTranscribing={isTranscribing}
        onClick={onClick}
      />
    );
  }

  return (
    <HeaderTabTranscriptActive
      isActive={isActive}
      isTranscribing={isTranscribing}
      canStopTranscription={canStopTranscription}
      onClick={onClick}
      sessionId={sessionId}
    />
  );
}

function HeaderTabTranscriptButton({
  isActive,
  isTranscribing,
  onClick,
  onContextMenu,
  onStopTranscription,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  onStopTranscription?: () => void;
}) {
  const { t } = useLingui();
  const handleClick = useCallback(() => {
    if (isTranscribing && onStopTranscription) {
      onStopTranscription();
      return;
    }

    onClick?.();
  }, [isTranscribing, onClick, onStopTranscription]);

  return (
    <IconHeaderTab
      isActive={isActive}
      label={t`Transcript`}
      icon={
        isTranscribing && onStopTranscription ? (
          <span className="group/stop relative flex size-4 items-center justify-center">
            <Spinner size={16} className="shrink-0 group-hover/stop:hidden" />
            <SquareIcon className="hidden size-3 fill-current group-hover/stop:block" />
          </span>
        ) : isTranscribing ? (
          <Spinner size={16} className="shrink-0" />
        ) : (
          <AudioLinesIcon className="size-4" />
        )
      }
      onClick={handleClick}
      onContextMenu={onContextMenu}
      title={
        isTranscribing && onStopTranscription ? "Stop transcription" : undefined
      }
    />
  );
}

function HeaderTabTranscriptActive({
  isActive,
  isTranscribing,
  canStopTranscription,
  onClick,
  sessionId,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  canStopTranscription: boolean;
  onClick?: () => void;
  sessionId: string;
}) {
  const regenerate = useRegenerateTranscript(sessionId);
  const stopTranscription = useListener((state) => state.stopTranscription);
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
  const handleStopTranscription = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);
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
    <HeaderTabTranscriptButton
      isActive={isActive}
      isTranscribing={isTranscribing}
      onClick={onClick}
      onContextMenu={showContextMenu}
      onStopTranscription={
        canStopTranscription ? handleStopTranscription : undefined
      }
    />
  );
}

function HeaderTabInsights({
  isActive,
  onClick = () => {},
}: {
  isActive: boolean;
  onClick?: () => void;
}) {
  const { t } = useLingui();

  return (
    <IconHeaderTab
      isActive={isActive}
      label={t`Insights`}
      icon={<LightbulbIcon className="size-4" />}
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
  onSelectTemplate,
  onRegenerateTemplate,
  selectedTemplateId,
  trigger,
}: {
  onSelectTemplate: (selection: TemplateSelection) => void;
  onRegenerateTemplate: (selection: TemplateSelection) => void;
  selectedTemplateId: string | null;
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
  const handleRegenerateTemplate = useCallback(
    (selection: TemplateSelection) => {
      setOpen(false);
      setSearch("");
      resultRefs.current = [];

      onRegenerateTemplate(selection);
    },
    [onRegenerateTemplate],
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
  const selectedTemplateExists = useMemo(() => {
    if (selectedTemplateId === null) {
      return true;
    }

    return (
      userTemplates.some((template) => template.id === selectedTemplateId) ||
      webTemplates.some((template) => template.slug === selectedTemplateId)
    );
  }, [selectedTemplateId, userTemplates, webTemplates]);
  const effectiveSelectedTemplateId = selectedTemplateExists
    ? selectedTemplateId
    : null;
  const templateItems = useMemo<
    Array<{
      key: string;
      title: string;
      isFavorite?: boolean;
      isSelected?: boolean;
      onClick: () => void;
      onRegenerate?: () => void;
    }>
  >(() => {
    const favoriteItems = filteredFavoriteTemplates.map((template) => ({
      key: template.id,
      title: template.title || "Untitled",
      isFavorite: true,
      isSelected: effectiveSelectedTemplateId === template.id,
      onClick: () =>
        handleUseTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
      onRegenerate: () =>
        handleRegenerateTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
    }));

    const userItems = filteredOtherTemplates.map((template) => ({
      key: template.id,
      title: template.title || "Untitled",
      isSelected: effectiveSelectedTemplateId === template.id,
      onClick: () =>
        handleUseTemplate({
          templateId: template.id,
          title: template.title || "Untitled",
        }),
      onRegenerate: () =>
        handleRegenerateTemplate({
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
    effectiveSelectedTemplateId,
    filteredFavoriteTemplates,
    filteredOtherTemplates,
    filteredWebTemplates,
    handleRegenerateTemplate,
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
        isSelected?: boolean;
        onClick: () => void;
        onRegenerate?: () => void;
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
          isSelected: effectiveSelectedTemplateId === null,
          onClick: () =>
            handleUseTemplate({
              templateId: null,
              title: "Auto",
            }),
          onRegenerate: () =>
            handleRegenerateTemplate({
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
  }, [
    handleCreateTemplate,
    handleRegenerateTemplate,
    handleUseTemplate,
    hasSearch,
    effectiveSelectedTemplateId,
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
                <div className="flex flex-col gap-1">
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
                              isSelected={item.isSelected}
                              onClick={item.onClick}
                              onRegenerate={item.onRegenerate}
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
              "flex h-7 w-full items-center justify-center gap-1 rounded-full px-3 text-xs font-medium",
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
  isTranscribing = false,
  canStopTranscription = false,
}: {
  sessionId: string;
  editorTabs: EditorView[];
  currentTab: EditorView;
  handleTabChange: (view: EditorView) => void;
  isTranscribing?: boolean;
  canStopTranscription?: boolean;
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
                ? "bg-foreground/10 dark:bg-accent/55 flex h-[30px] items-center gap-[2px] rounded-full p-[2px]"
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

              if (view.type === "insights") {
                return (
                  <HeaderTabInsights
                    key={view.type}
                    isActive={currentTab.type === view.type}
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
                    isTranscribing={isTranscribing}
                    canStopTranscription={canStopTranscription}
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
  audioExists = false,
  sessionId,
}: {
  audioExists?: boolean;
  sessionId: string;
}): EditorView[] {
  useEnsureDefaultSummary(sessionId);
  const canShowTranscript = useCanShowTranscript(sessionId, { audioExists });
  const canShowInsights = useCanShowInsights(sessionId);

  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );

  return createEditorTabs({
    enhancedNoteIds: enhancedNoteIds || [],
    canShowInsights,
    canShowTranscript,
  });
}

function createEditorTabs({
  enhancedNoteIds,
  canShowInsights,
  canShowTranscript,
}: {
  enhancedNoteIds: string[];
  canShowInsights: boolean;
  canShowTranscript: boolean;
}): EditorView[] {
  const enhancedTabs: EditorView[] = enhancedNoteIds.map((id) => ({
    type: "enhanced",
    id,
  }));

  return [
    ...enhancedTabs,
    ...(canShowInsights ? [{ type: "insights" } as const] : []),
    { type: "raw" },
    ...(canShowTranscript ? [{ type: "transcript" } as const] : []),
  ];
}

const useEnhanceLogic = (sessionId: string, enhancedNoteId: string) =>
  useEnhancedNoteActions({ sessionId, enhancedNoteId });

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
      <div className="flex flex-col gap-0.5">{children}</div>
    </div>
  );
}

function TemplateResultButton({
  buttonRef,
  title,
  isFavorite = false,
  isSelected = false,
  onClick,
  onRegenerate,
  onKeyDown,
}: {
  buttonRef?: React.Ref<HTMLButtonElement>;
  title: string;
  isFavorite?: boolean;
  isSelected?: boolean;
  onClick: () => void;
  onRegenerate?: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLButtonElement>) => void;
}) {
  return (
    <div
      className={cn([
        "group/template-row hover:bg-accent focus-within:bg-muted flex h-9 w-full items-center rounded-full px-3 transition-colors",
        isSelected ? "bg-accent/70" : null,
      ])}
    >
      <button
        ref={buttonRef}
        aria-current={isSelected ? "true" : undefined}
        className="flex h-full min-w-0 flex-1 items-center gap-1.5 text-left focus:outline-hidden"
        onClick={onClick}
        onKeyDown={onKeyDown}
      >
        {isSelected ? <CheckIcon className="size-3.5 shrink-0" /> : null}
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
      {isSelected && onRegenerate ? (
        <button
          type="button"
          aria-label={`Regenerate ${title}`}
          title="Regenerate summary"
          className={cn([
            "text-muted-foreground hover:text-foreground focus:text-foreground",
            "ml-2 flex size-6 shrink-0 items-center justify-center rounded-sm transition-colors focus:outline-hidden",
          ])}
          onClick={(event) => {
            event.stopPropagation();
            onRegenerate();
          }}
        >
          <RefreshCwIcon className="size-3.5" />
        </button>
      ) : null}
    </div>
  );
}
