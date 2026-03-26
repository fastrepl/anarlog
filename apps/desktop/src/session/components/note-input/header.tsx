import { useQuery } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  CheckIcon,
  CopyIcon,
  PlusIcon,
  RefreshCwIcon,
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
import { getEnhancerService } from "~/services/enhancer";
import { useHasTranscript } from "~/session/components/shared";
import { useEnsureDefaultSummary } from "~/session/hooks/useEnhancedNotes";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { type TaskStepInfo } from "~/store/zustand/ai-task/tasks";
import { useTabs } from "~/store/zustand/tabs";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useRunBatch } from "~/stt/useRunBatch";

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
  const templates = main.UI.useResultTable(
    main.QUERIES.visibleTemplates,
    main.STORE_ID,
  );
  const openNew = useTabs((state) => state.openNew);

  const handleTemplateClick = useCallback(
    (templateId: string) => {
      setOpen(false);

      const service = getEnhancerService();
      if (!service) return;

      const result = service.enhance(sessionId, { templateId });
      if (result.type === "started" || result.type === "already_active") {
        handleTabChange({ type: "enhanced", id: result.noteId });
      }
    },
    [sessionId, handleTabChange],
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
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
          <span>Create other format</span>
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-64" align="start">
        <div className="flex flex-col gap-2">
          {Object.entries(templates).length > 0 ? (
            <>
              {Object.entries(templates).map(([templateId, template]) => (
                <TemplateButton
                  key={templateId}
                  onClick={() => handleTemplateClick(templateId)}
                >
                  {template.title}
                </TemplateButton>
              ))}
              <TemplateButton
                className="text-neutral-500 italic hover:bg-neutral-50 hover:text-neutral-700"
                onClick={() => {
                  setOpen(false);
                  openNew({ type: "settings", state: { tab: "templates" } });
                }}
              >
                Manage templates
              </TemplateButton>
            </>
          ) : (
            <>
              <p className="mb-2 text-center text-sm text-neutral-600">
                No templates yet
              </p>
              <button
                onClick={() => {
                  setOpen(false);
                  openNew({ type: "settings", state: { tab: "templates" } });
                }}
                className="rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 py-2 text-sm font-medium text-white transition-opacity duration-150 hover:opacity-90"
              >
                Create templates
              </button>
            </>
          )}
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

function TemplateButton({
  children,
  onClick,
  className,
}: {
  children: React.ReactNode;
  onClick: () => void;
  className?: string;
}) {
  return (
    <button
      className={cn([
        "rounded-md px-3 py-2 text-center text-sm transition-colors hover:bg-neutral-100",
        className,
      ])}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
