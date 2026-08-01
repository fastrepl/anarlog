import { useLingui } from "@lingui/react/macro";
import {
  CaretDown,
  Sparkle,
  TextAlignLeft,
  Waveform,
} from "@phosphor-icons/react";
import { useCallback, useMemo } from "react";

import { json2md, parseJsonContent } from "@anlg/editor/markdown";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import {
  TemplatePickerPopover,
  type TemplateSelection,
} from "./template-picker";

import { useAITaskTask } from "~/ai/hooks";
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
import {
  deleteEnhancedNote,
  useEnhancedNote,
  useEnhancedNoteRecords,
  useSession,
} from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useUserTemplate } from "~/templates";

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

function IconHeaderView({
  isActive,
  label,
  hoverLabel,
  icon,
  onClick,
  onContextMenu,
  title,
  size = "tray",
  className,
}: {
  isActive: boolean;
  label: string;
  hoverLabel?: string;
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
      data-hover-label={hoverLabel}
      className={iconHeaderViewClassName(
        isActive,
        size,
        cn([
          "px-2",
          isActive ? "max-w-40 min-w-10 gap-1.5" : null,
          hoverLabel
            ? "after:hidden after:min-w-0 after:truncate after:text-xs after:font-medium after:content-[attr(data-hover-label)] hover:after:block"
            : null,
          className,
        ]),
      )}
    >
      {icon}
      {isActive && (
        <span
          className={cn([
            "min-w-0 truncate text-xs font-medium",
            hoverLabel ? "group-hover/header-view:hidden" : null,
          ])}
        >
          {label}
        </span>
      )}
    </button>
  );
}

function iconHeaderViewClassName(
  isActive: boolean,
  size: "tray" | "standalone" = "tray",
  className?: string,
) {
  const heightClassName = size === "tray" ? "h-[26px]" : "h-7";

  return cn([
    "group/header-view flex shrink-0 items-center justify-center rounded-full transition-colors select-none [&>svg]:shrink-0",
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
    console.error("Failed to copy note content", error);

    if (messages) {
      sonnerToast.error(messages.error);
    }

    return false;
  }
}

function HeaderViewRaw({
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
      <HeaderViewRawButton
        isActive={isActive}
        onClick={onClick}
        standalone={standalone}
      />
    );
  }

  return (
    <HeaderViewRawActive
      isActive={isActive}
      onClick={onClick}
      sessionId={sessionId}
      standalone={standalone}
    />
  );
}

function HeaderViewRawButton({
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
    <IconHeaderView
      isActive={isActive}
      label={t`Memos`}
      icon={<TextAlignLeft className="size-4" />}
      onClick={onClick}
      onContextMenu={onContextMenu}
      size={standalone ? "standalone" : "tray"}
      className={standalone ? "border-0 shadow-none" : undefined}
    />
  );
}

function HeaderViewRawActive({
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
  const rawMd = useSession(sessionId)?.raw_md;
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
    <HeaderViewRawButton
      isActive={isActive}
      onClick={onClick}
      onContextMenu={showContextMenu}
      standalone={standalone}
    />
  );
}

function HeaderViewEnhanced({
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
      <HeaderViewEnhancedInactive
        enhancedNoteId={enhancedNoteId}
        onClick={onClick}
      />
    );
  }

  return (
    <HeaderViewEnhancedActive
      sessionId={sessionId}
      enhancedNoteId={enhancedNoteId}
      canRemove={canRemove}
      onRemove={onRemove}
      onSelectNote={onSelectNote}
    />
  );
}

function useEnhancedViewTitle(enhancedNoteId: string) {
  const enhancedNote = useEnhancedNote(enhancedNoteId);
  const rawTitle = enhancedNote?.title;
  const templateId = enhancedNote?.templateId;
  const { data: template } = useUserTemplate(templateId);
  const templateTitle = template?.title?.trim() || null;
  const viewTitle = getEnhancedNoteTitle({
    rawTitle,
    templateTitle,
    templateId,
  });

  return {
    viewTitle,
    templateTooltip:
      templateId && templateTitle
        ? `${templateTitle} was used to generate this summary.`
        : undefined,
  };
}

function useEnhancedViewGenerating(enhancedNoteId: string) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const enhanceTask = useAITaskTask(taskId, "enhance");

  return enhanceTask.isGenerating;
}

function HeaderViewEnhancedInactive({
  onClick = () => {},
  enhancedNoteId,
}: {
  enhancedNoteId: string;
  onClick?: () => void;
}) {
  const { viewTitle, templateTooltip } = useEnhancedViewTitle(enhancedNoteId);
  const isGenerating = useEnhancedViewGenerating(enhancedNoteId);

  return (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={viewTitle}
      onClick={onClick}
      title={templateTooltip}
      className={iconHeaderViewClassName(false, "tray", "px-2")}
    >
      {isGenerating ? (
        <Spinner size={16} className="shrink-0" />
      ) : (
        <Sparkle className="size-4" />
      )}
    </button>
  );
}

function HeaderViewEnhancedActive({
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
  const { isGenerating, isError, onRegenerate } = useEnhanceLogic(
    sessionId,
    enhancedNoteId,
  );
  const content = useEnhancedNote(enhancedNoteId)?.content;
  const { viewTitle, templateTooltip } = useEnhancedViewTitle(enhancedNoteId);
  const noteMarkdown = useMemo(() => getStoredNoteMarkdown(content), [content]);

  const handleCopy = useCallback(() => {
    return copyTextToClipboard(noteMarkdown, {
      success: `${viewTitle} copied to clipboard`,
      error: `Failed to copy ${viewTitle}`,
    });
  }, [noteMarkdown, viewTitle]);
  const handleRegenerate = useCallback(() => {
    void onRegenerate(null);
  }, [onRegenerate]);
  const handleSelectTemplate = useCallback(
    (selection: TemplateSelection) => {
      if (isGenerating) {
        return;
      }

      const service = getEnhancerService();
      if (!service) {
        return;
      }

      onSelectNote?.(enhancedNoteId);

      void Promise.resolve(
        service.enhance(sessionId, {
          templateId: selection.templateId,
          targetNoteId: enhancedNoteId,
          templateTitle: selection.templateId ? selection.title : undefined,
        }),
      )
        .then((result) => {
          if (
            (result.type === "started" || result.type === "already_active") &&
            result.noteId !== enhancedNoteId
          ) {
            onSelectNote?.(result.noteId);
          }
        })
        .catch((error) => {
          console.error("[enhancer] failed to replace summary template", error);
        });
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
  const templateMenuTrigger = (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={viewTitle}
      aria-current="page"
      aria-disabled={isGenerating}
      tabIndex={isGenerating ? -1 : 0}
      onClick={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={showContextMenu}
      title={templateTooltip}
      className={iconHeaderViewClassName(
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
        <Sparkle className="size-4" />
      )}
      <span className="min-w-0 truncate text-xs font-medium">{viewTitle}</span>
      <CaretDown className="size-3.5" />
    </button>
  );

  return (
    <TemplatePickerPopover
      onSelectTemplate={handleSelectTemplate}
      trigger={templateMenuTrigger}
    />
  );
}

function HeaderViewTranscript({
  isActive,
  isTranscribing,
  onClick = () => {},
  sessionId,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  sessionId: string;
}) {
  const liveState = useTranscriptLiveViewState(sessionId);

  if (!isActive) {
    return (
      <HeaderViewTranscriptButton
        isActive={isActive}
        isTranscribing={isTranscribing}
        onClick={onClick}
        live={liveState.live}
      />
    );
  }

  return (
    <HeaderViewTranscriptActive
      isActive={isActive}
      isTranscribing={isTranscribing}
      onClick={onClick}
      sessionId={sessionId}
      live={liveState.live}
    />
  );
}

function HeaderViewTranscriptButton({
  isActive,
  isTranscribing,
  onClick,
  onContextMenu,
  live,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  live?: {
    amplitude: number;
    degraded: boolean;
    muted: boolean;
  };
}) {
  const { t } = useLingui();

  return (
    <IconHeaderView
      isActive={isActive}
      label={t`Transcript`}
      hoverLabel={undefined}
      icon={
        live ? (
          <HeaderViewTranscriptLiveIcon live={live} />
        ) : isTranscribing ? (
          <Spinner size={16} className="shrink-0" />
        ) : (
          <Waveform className="size-4" />
        )
      }
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={undefined}
      className={cn([
        live
          ? [
              "group/transcript-live",
              isActive ? "w-[98px] min-w-[98px] gap-1.5 pr-1.5 pl-2" : null,
              isActive
                ? live.degraded
                  ? [
                      "bg-amber-50 text-amber-500 hover:bg-amber-100 hover:text-amber-600",
                      "dark:bg-amber-950/50 dark:text-amber-300 dark:hover:bg-amber-950 dark:hover:text-amber-200",
                    ]
                  : [
                      "bg-red-50 text-red-500 hover:bg-red-100 hover:text-red-600",
                      "dark:bg-red-950/50 dark:text-red-300 dark:hover:bg-red-950 dark:hover:text-red-200",
                    ]
                : null,
            ]
          : null,
      ])}
    />
  );
}

function HeaderViewTranscriptLiveIcon({
  live,
}: {
  live: {
    amplitude: number;
    degraded: boolean;
    muted: boolean;
  };
}) {
  const color = live.degraded ? "#f59e0b" : "#ef4444";

  return (
    <span className="relative flex size-4 items-center justify-center">
      {live.muted ? (
        <Waveform className="size-4" />
      ) : (
        <DancingSticks
          amplitude={live.amplitude}
          color={color}
          height={16}
          width={16}
        />
      )}
    </span>
  );
}

function useTranscriptLiveViewState(sessionId: string) {
  const { amplitude, degraded, mode, muted } = useListener((state) => {
    const mode = state.getSessionMode(sessionId);
    return {
      amplitude: state.live.amplitude,
      degraded: state.live.degraded,
      mode,
      muted: state.live.muted,
    };
  });
  return {
    live:
      mode === "active"
        ? {
            amplitude: Math.min(
              Math.hypot(amplitude.mic, amplitude.speaker),
              1,
            ),
            degraded: Boolean(degraded),
            muted,
          }
        : undefined,
  };
}

function HeaderViewTranscriptActive({
  isActive,
  isTranscribing,
  onClick,
  sessionId,
  live,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  sessionId: string;
  live?: {
    amplitude: number;
    degraded: boolean;
    muted: boolean;
  };
}) {
  const regenerate = useRegenerateTranscript(sessionId);
  const { request: transcriptExportRequest } =
    useSessionTranscriptRenderData(sessionId);
  const {
    audioExists,
    audioExistsResolved,
    deleteRecording,
    isDeletingRecording,
  } = AudioPlayer.useAudioPlayer();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
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

    if (audioExistsResolved && sessionMode === "inactive" && audioExists) {
      items.push({
        id: `regenerate-transcript-${sessionId}`,
        text: "Re-transcribe",
        action: () => {
          void regenerate();
        },
      });
    }

    if (audioExists) {
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
    audioExistsResolved,
    canCopyTranscript,
    handleCopyTranscript,
    handleDeleteRecording,
    isDeletingRecording,
    regenerate,
    sessionMode,
    sessionId,
  ]);
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <HeaderViewTranscriptButton
      isActive={isActive}
      isTranscribing={isTranscribing}
      onClick={onClick}
      onContextMenu={showContextMenu}
      live={live}
    />
  );
}

export function Header({
  sessionId,
  editorTabs,
  currentTab,
  handleTabChange,
  isTranscribing = false,
}: {
  sessionId: string;
  editorTabs: EditorView[];
  currentTab: EditorView;
  handleTabChange: (view: EditorView) => void;
  isTranscribing?: boolean;
}) {
  const { t } = useLingui();
  const primaryEnhancedTabId = editorTabs.find(
    (view): view is Extract<EditorView, { type: "enhanced" }> =>
      view.type === "enhanced",
  )?.id;
  const shouldUseViewSwitcher = editorTabs.length > 1;

  return (
    <div data-tauri-drag-region className="flex flex-col pl-1">
      <div
        data-tauri-drag-region
        className="flex items-center justify-between gap-2"
      >
        <div data-tauri-drag-region className="relative min-w-0 flex-1">
          <div
            role="group"
            aria-label={t`Session note views`}
            data-tauri-drag-region="false"
            className={cn([
              "pointer-events-auto relative z-10 w-fit max-w-full overflow-visible",
              shouldUseViewSwitcher
                ? "bg-foreground/10 dark:bg-accent/55 flex h-[30px] items-center gap-[2px] rounded-full p-[2px]"
                : null,
            ])}
          >
            {editorTabs.map((view, index) => {
              if (view.type === "enhanced") {
                return (
                  <HeaderViewEnhanced
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

                            void deleteEnhancedNote(view.id).catch((error) => {
                              console.error(
                                "[session-header] failed to remove summary",
                                error,
                              );
                            });
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
                  <HeaderViewRaw
                    key={view.type}
                    sessionId={sessionId}
                    isActive={currentTab.type === view.type}
                    standalone={!shouldUseViewSwitcher}
                    onClick={() => handleTabChange(view)}
                  />
                );
              }

              if (view.type === "transcript") {
                return (
                  <HeaderViewTranscript
                    key={view.type}
                    sessionId={sessionId}
                    isActive={currentTab.type === view.type}
                    isTranscribing={isTranscribing}
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

  const enhancedNoteIds = useEnhancedNoteRecords(sessionId).map(
    (note) => note.id,
  );

  return createEditorTabs({
    enhancedNoteIds,
    canShowTranscript,
  });
}

export function createEditorTabs({
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

const useEnhanceLogic = (sessionId: string, enhancedNoteId: string) =>
  useEnhancedNoteActions({ sessionId, enhancedNoteId });
