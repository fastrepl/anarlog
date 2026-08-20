import { useLingui } from "@lingui/react/macro";
import { CaretDown, Square } from "@phosphor-icons/react";
import { useCallback } from "react";

import { cn } from "@anlg/utils";

import { iconHeaderViewClassName } from "./header-shared";

import { MetadataButton } from "~/session/components/outer-header/metadata";
import { useListener } from "~/stt/contexts";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function isMeetingStopAction(sessionMode: string) {
  return sessionMode === "active" || sessionMode === "running_batch";
}

export function HeaderViewStop({ sessionId }: { sessionId: string }) {
  const { t } = useLingui();
  const { sessionMode, stop, stopTranscription } = useListener((state) => ({
    sessionMode: state.getSessionMode(sessionId),
    stop: state.stop,
    stopTranscription: state.stopTranscription,
  }));
  const stopListening = useCallback(() => {
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl("stop", sessionId);
      return;
    }

    stop();
  }, [sessionId, stop]);
  const isBatch = sessionMode === "running_batch";
  const label = t`Stop`;
  const title = isBatch ? t`Stop transcription` : t`Stop listening`;

  return (
    <div className="flex h-[26px] min-w-0 items-center">
      <button
        type="button"
        data-tauri-drag-region="false"
        aria-label={label}
        title={title}
        onClick={
          isBatch
            ? () => {
                void stopTranscription(sessionId);
              }
            : stopListening
        }
        className={iconHeaderViewClassName(
          false,
          "tray",
          "text-foreground max-w-40 min-w-10 gap-1.5 px-2 hover:text-foreground",
        )}
      >
        <Square className="size-3 text-red-500" weight="fill" />
        <span className="min-w-0 truncate text-xs font-medium">{label}</span>
      </button>
      <MetadataButton
        sessionId={sessionId}
        renderTrigger={({ open, label: metadataLabel }) => (
          <button
            type="button"
            data-tauri-drag-region="false"
            aria-label={metadataLabel}
            title={metadataLabel}
            className={cn([
              "flex h-full w-5 shrink-0 items-center justify-center rounded-full transition-colors",
              "text-muted-foreground/70",
              "hover:bg-background/60 hover:text-foreground",
              "dark:hover:bg-accent/80 dark:hover:text-foreground",
              open &&
                "bg-background/60 text-foreground dark:bg-accent/80 dark:text-foreground",
            ])}
          >
            <CaretDown size={14} />
          </button>
        )}
      />
    </div>
  );
}
