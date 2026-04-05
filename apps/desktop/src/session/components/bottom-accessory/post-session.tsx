import { Pencil, RefreshCw } from "lucide-react";
import { useCallback, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { ExpandToggle } from "./expand-toggle";

import * as AudioPlayer from "~/audio-player";
import { Transcript } from "~/session/components/note-input/transcript";
import { useTranscriptScreen } from "~/session/components/note-input/transcript/state";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";
import { useRunBatch } from "~/stt/useRunBatch";
import { useUploadFile } from "~/stt/useUploadFile";

export function PostSessionAccessory({
  sessionId,
  hasAudio,
  hasTranscript,
  isTranscriptExpanded,
  onToggleTranscript,
}: {
  sessionId: string;
  hasAudio: boolean;
  hasTranscript: boolean;
  isTranscriptExpanded: boolean;
  onToggleTranscript: () => void;
}) {
  return (
    <div className="flex flex-col gap-1">
      <TranscriptPanel
        sessionId={sessionId}
        hasTranscript={hasTranscript}
        isExpanded={isTranscriptExpanded}
        onToggleExpand={onToggleTranscript}
      />
      {hasAudio && <AudioPlayer.Timeline />}
    </div>
  );
}

function TranscriptPanel({
  sessionId,
  hasTranscript,
  isExpanded,
  onToggleExpand,
}: {
  sessionId: string;
  hasTranscript: boolean;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  if (hasTranscript) {
    return (
      <TranscriptReadyPanel
        sessionId={sessionId}
        isExpanded={isExpanded}
        onToggleExpand={onToggleExpand}
      />
    );
  }

  return (
    <TranscriptEmptyPanel
      sessionId={sessionId}
      isExpanded={isExpanded}
      onToggleExpand={onToggleExpand}
    />
  );
}

function useRegenerateTranscript(sessionId: string) {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const runBatch = useRunBatch(sessionId);
  const handleBatchStarted = useListener((state) => state.handleBatchStarted);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);

  return useCallback(async () => {
    if (!store) return;

    const result = await fsSyncCommands.audioPath(sessionId);
    if (result.status === "error") return;

    const audioPath = result.data;

    if (indexes) {
      const transcriptIds = indexes.getSliceRowIds(
        main.INDEXES.transcriptBySession,
        sessionId,
      );
      store.transaction(() => {
        for (const id of transcriptIds) {
          store.delRow("transcripts", id);
        }
      });
    }

    handleBatchStarted(sessionId, "transcribing");

    try {
      await runBatch(audioPath);
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      handleBatchFailed(sessionId, msg);
    }
  }, [
    handleBatchFailed,
    handleBatchStarted,
    indexes,
    runBatch,
    sessionId,
    store,
  ]);
}

function TranscriptReadyPanel({
  sessionId,
  isExpanded,
  onToggleExpand,
}: {
  sessionId: string;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const screen = useTranscriptScreen({ sessionId });
  const regenerate = useRegenerateTranscript(sessionId);
  const isBatching = screen.kind === "running_batch";
  const percentage = isBatching ? screen.percentage : undefined;
  const phase = isBatching ? screen.phase : undefined;

  return (
    <div className="relative w-full pt-3 select-none">
      <ExpandToggle
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        label="Transcript"
      />

      {isExpanded && (
        <div className="rounded-xl bg-neutral-50">
          <div className="flex items-center justify-between rounded-t-xl border-b border-neutral-200 bg-neutral-100 px-3 py-1.5">
            <span className="text-xs font-medium text-neutral-500">
              Transcript
            </span>

            {isBatching ? (
              <div className="flex items-center gap-1.5 px-1 py-0.5">
                <Spinner size={10} />
                <span className="text-[11px] text-neutral-500">
                  {phase === "importing" ? "Importing..." : "Regenerating..."}
                  {typeof percentage === "number" && percentage > 0 && (
                    <span className="ml-1 text-neutral-400 tabular-nums">
                      {Math.round(percentage * 100)}%
                    </span>
                  )}
                </span>
              </div>
            ) : (
              <div className="flex items-center gap-1">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <button
                      type="button"
                      disabled
                      className={cn([
                        "flex items-center gap-1 rounded px-1.5 py-0.5",
                        "text-[11px] font-medium text-neutral-300",
                        "cursor-not-allowed",
                      ])}
                    >
                      <Pencil size={10} />
                      Edit
                    </button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    <p>Coming soon</p>
                  </TooltipContent>
                </Tooltip>
                <button
                  type="button"
                  onClick={regenerate}
                  className={cn([
                    "flex items-center gap-1 rounded px-1.5 py-0.5",
                    "text-[11px] font-medium text-neutral-500",
                    "transition-colors hover:bg-neutral-200/60 hover:text-neutral-700",
                  ])}
                >
                  <RefreshCw size={10} />
                  Regenerate
                </button>
              </div>
            )}
          </div>

          <div className="h-[300px] overflow-y-auto">
            <Transcript sessionId={sessionId} scrollRef={scrollRef} />
          </div>
        </div>
      )}
    </div>
  );
}

function TranscriptEmptyPanel({
  sessionId,
  isExpanded,
  onToggleExpand,
}: {
  sessionId: string;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const { uploadAudio } = useUploadFile(sessionId);
  const regenerate = useRegenerateTranscript(sessionId);

  const isBatching = screen.kind === "running_batch";
  const percentage = isBatching ? screen.percentage : undefined;
  const phase = isBatching ? screen.phase : undefined;
  const error = screen.kind === "empty" ? screen.error : null;
  const hasAudio = screen.kind === "empty" ? screen.hasAudio : false;

  return (
    <div className="relative w-full pt-3 select-none">
      <ExpandToggle
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        label="Transcript"
      />

      {isExpanded && (
        <div className="rounded-xl bg-neutral-50">
          {isBatching ? (
            <div className="flex h-[120px] flex-col items-center justify-center gap-2">
              <Spinner size={24} />
              {typeof percentage === "number" && percentage > 0 && (
                <p className="text-xl font-medium text-neutral-500 tabular-nums">
                  {Math.round(percentage * 100)}%
                </p>
              )}
              <p className="text-sm text-neutral-400">
                {phase === "importing"
                  ? "Importing audio..."
                  : "Generating transcript..."}
              </p>
            </div>
          ) : (
            <div className="flex items-center justify-between px-4 py-3">
              {error ? (
                <span className="text-xs text-red-500">{error}</span>
              ) : (
                <span className="text-xs text-neutral-400">
                  No transcript yet
                </span>
              )}

              <div className="flex items-center gap-1.5">
                {hasAudio && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 gap-1.5 text-xs text-neutral-500"
                    onClick={regenerate}
                  >
                    <RefreshCw size={12} />
                    Generate
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 text-xs text-neutral-500"
                  onClick={uploadAudio}
                >
                  Upload audio
                </Button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
