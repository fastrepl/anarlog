import { Trans, useLingui } from "@lingui/react/macro";
import { useCallback, useState } from "react";

import { DotsThreeVertical } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { appFloatingItemClassName } from "@anlg/ui/components/ui/floating-content";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";
import { cn } from "@anlg/utils";

import { ActionableTooltipContent } from "./shared";

import { useUploadFile } from "~/stt/useUploadFile";

export function OptionsMenu({
  sessionId,
  disabled,
  warningMessage,
  hideUploadActions = false,
  onConfigure,
  children,
}: {
  sessionId: string;
  disabled: boolean;
  warningMessage: string;
  hideUploadActions?: boolean;
  onConfigure?: () => void;
  children: React.ReactNode;
}) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);

  const handleUploadAudio = useCallback(() => {
    if (disabled) {
      return;
    }
    setOpen(false);
    uploadAudio();
  }, [disabled, uploadAudio]);

  const handleUploadTranscript = useCallback(() => {
    if (disabled) {
      return;
    }
    setOpen(false);
    uploadTranscript();
  }, [disabled, uploadTranscript]);

  const moreButton = (
    <button
      className="text-primary-foreground/70 hover:text-primary-foreground dark:text-primary/65 dark:hover:text-primary absolute top-1/2 right-2 -translate-y-1/2 cursor-pointer transition-colors disabled:opacity-50"
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        setOpen(true);
      }}
    >
      <DotsThreeVertical className="size-4" />
      <span className="sr-only">
        <Trans>More options</Trans>
      </span>
    </button>
  );

  if (hideUploadActions) {
    return <div className="relative flex items-center">{children}</div>;
  }

  if (disabled && warningMessage) {
    return (
      <div className="relative flex items-center">
        {children}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span className="inline-block">{moreButton}</span>
          </TooltipTrigger>
          <TooltipContent side="top" align="end">
            <ActionableTooltipContent
              message={warningMessage}
              action={
                onConfigure
                  ? {
                      label: t`Configure`,
                      handleClick: onConfigure,
                    }
                  : undefined
              }
            />
          </TooltipContent>
        </Tooltip>
      </div>
    );
  }

  if (disabled) {
    return (
      <div className="relative flex items-center">
        {children}
        {moreButton}
      </div>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div className="relative flex items-center">
          {children}
          {moreButton}
        </div>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        side="top"
        align="center"
        sideOffset={8}
        className="w-43"
      >
        <AppFloatingPanel
          className={cn([appFloatingMenuPanelClassName, "flex flex-col gap-1"])}
        >
          <Button
            variant="ghost"
            className={cn([
              "h-9 justify-center px-3 whitespace-nowrap",
              appFloatingItemClassName,
            ])}
            onClick={handleUploadAudio}
          >
            <span className="text-sm">
              <Trans>Upload audio</Trans>
            </span>
          </Button>
          <Button
            variant="ghost"
            className={cn([
              "h-9 justify-center px-3 whitespace-nowrap",
              appFloatingItemClassName,
            ])}
            onClick={handleUploadTranscript}
          >
            <span className="text-sm">
              <Trans>Upload transcript</Trans>
            </span>
          </Button>
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}
