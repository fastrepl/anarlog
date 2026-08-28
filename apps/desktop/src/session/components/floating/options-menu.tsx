import { Trans, useLingui } from "@lingui/react/macro";
import { DotsThreeVertical } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

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
      {...stylex.props(styles.moreButton)}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        setOpen(true);
      }}
    >
      <DotsThreeVertical {...stylex.props(styles.icon)} />
      <span {...stylex.props(styles.visuallyHidden)}>
        <Trans>More options</Trans>
      </span>
    </button>
  );

  if (hideUploadActions) {
    return <div {...stylex.props(styles.container)}>{children}</div>;
  }

  if (disabled && warningMessage) {
    return (
      <div {...stylex.props(styles.container)}>
        {children}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span {...stylex.props(styles.inlineBlock)}>{moreButton}</span>
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
      <div {...stylex.props(styles.container)}>
        {children}
        {moreButton}
      </div>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div {...stylex.props(styles.container)}>
          {children}
          {moreButton}
        </div>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        side="top"
        align="center"
        sideOffset={8}
        sx={styles.popover}
      >
        <AppFloatingPanel sx={styles.panel}>
          <Button
            variant="ghost"
            sx={styles.action}
            onClick={handleUploadAudio}
          >
            <span {...stylex.props(styles.actionLabel)}>
              <Trans>Upload audio</Trans>
            </span>
          </Button>
          <Button
            variant="ghost"
            sx={styles.action}
            onClick={handleUploadTranscript}
          >
            <span {...stylex.props(styles.actionLabel)}>
              <Trans>Upload transcript</Trans>
            </span>
          </Button>
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

const styles = stylex.create({
  action: {
    height: "2.25rem",
    justifyContent: "center",
    paddingInline: "0.75rem",
    whiteSpace: "nowrap",
  },
  actionLabel: {
    fontSize: "0.875rem",
  },
  container: {
    alignItems: "center",
    display: "flex",
    position: "relative",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  inlineBlock: {
    display: "inline-block",
  },
  moreButton: {
    color: {
      default: `color-mix(in srgb, ${colors.primaryForeground} 70%, transparent)`,
      ":hover": colors.primaryForeground,
    },
    cursor: "pointer",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    position: "absolute",
    right: "0.5rem",
    top: "50%",
    transform: "translateY(-50%)",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
    padding: "0.25rem",
  },
  popover: {
    width: "10.75rem",
  },
  visuallyHidden: {
    borderWidth: 0,
    clipPath: "inset(50%)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
});
