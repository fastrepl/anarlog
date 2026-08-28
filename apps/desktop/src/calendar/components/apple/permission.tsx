import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  WarningCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import {
  colors,
  media,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
import { type PermissionStatus } from "@anlg/plugin-permissions";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";

export function AppleCalendarPermissionDialog({
  open,
  onOpenChange,
  onOpenSettings,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onOpenSettings: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader sx={styles.dialogHeader}>
          <DialogTitle sx={styles.dialogTitle}>
            <Trans>Apple Calendar access is off</Trans>
          </DialogTitle>
          <DialogDescription sx={styles.dialogDescription}>
            <Trans>
              Turn on Anarlog in System Settings → Privacy &amp; Security →
              Calendars, then return here.
            </Trans>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter sx={styles.dialogFooter}>
          <GlassDialogCancelButton onClick={() => onOpenChange(false)}>
            <Trans>Cancel</Trans>
          </GlassDialogCancelButton>
          <Button
            sx={styles.dialogAction}
            onClick={() => {
              onOpenSettings();
              onOpenChange(false);
            }}
          >
            <Trans>Open Settings</Trans>
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}

function ActionLink({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      {...stylex.props([styles.actionLink, disabled && styles.disabled])}
    >
      {children}
    </button>
  );
}

export function AccessPermissionRow({
  title,
  status,
  isPending,
  onOpen,
  onRequest,
  onReset,
  showActionButton = true,
}: {
  title: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  onOpen: () => void;
  onRequest: () => void;
  onReset: () => void;
  showActionButton?: boolean;
}) {
  const { t } = useLingui();
  const isAuthorized = status === "authorized";
  const isDenied = status === "denied";

  const handleButtonClick = () => {
    if (isAuthorized || isDenied) {
      onOpen();
    } else {
      onRequest();
    }
  };

  return (
    <div
      {...stylex.props([
        styles.permissionRow,
        showActionButton ? styles.rowWithAction : styles.rowWithoutAction,
      ])}
    >
      <div {...stylex.props(styles.permissionCopy)}>
        <div
          {...stylex.props([
            styles.permissionTitle,
            !isAuthorized && styles.unauthorizedTitle,
          ])}
        >
          {!isAuthorized && <WarningCircle {...stylex.props(styles.icon)} />}
          <h3 {...stylex.props(styles.heading)}>{title}</h3>
        </div>
        <TroubleShootingLink
          onRequest={onRequest}
          onReset={onReset}
          onOpen={onOpen}
          isPending={isPending}
        />
      </div>
      {showActionButton && (
        <Button
          variant={isAuthorized ? "outline" : "default"}
          size="icon"
          onClick={handleButtonClick}
          disabled={isPending}
          sx={[
            styles.permissionAction,
            isAuthorized && styles.authorizedAction,
          ]}
          aria-label={
            isAuthorized
              ? t`Open ${title.toLowerCase()} settings`
              : t`Request ${title.toLowerCase()}`
          }
        >
          {isAuthorized ? (
            <Check {...stylex.props(styles.actionIcon)} />
          ) : (
            <ArrowRight {...stylex.props(styles.actionIcon)} />
          )}
        </Button>
      )}
    </div>
  );
}

export function TroubleShootingLink({
  onRequest,
  onReset,
  onOpen,
  isPending,
  sx,
}: {
  onRequest: () => void;
  onReset: () => void;
  onOpen: () => void;
  isPending: boolean;
  sx?: stylex.StyleXStyles;
}) {
  const { t } = useLingui();
  const [showActions, setShowActions] = useState(false);
  return (
    <div {...stylex.props([styles.troubleshooting, sx])}>
      {!showActions ? (
        <button
          type="button"
          onClick={() => setShowActions(true)}
          {...stylex.props(styles.actionLink)}
        >
          <Trans>Having trouble?</Trans>
        </button>
      ) : (
        <div>
          <Trans>You can</Trans>{" "}
          <ActionLink onClick={onRequest} disabled={isPending}>
            {t`Request`},
          </ActionLink>{" "}
          <ActionLink onClick={onReset} disabled={isPending}>
            <Trans>Reset</Trans>
          </ActionLink>{" "}
          <Trans>or</Trans>{" "}
          <ActionLink onClick={onOpen} disabled={isPending}>
            <Trans>Open</Trans>
          </ActionLink>{" "}
          <Trans>permission panel.</Trans>{" "}
          <ActionLink onClick={() => setShowActions(false)}>
            <ArrowLeft {...stylex.props(styles.backIcon)} />
            <Trans>Back</Trans>
          </ActionLink>
        </div>
      )}
    </div>
  );
}

const styles = stylex.create({
  actionIcon: {
    height: "1.25rem",
    width: "1.25rem",
  },
  actionLink: {
    color: {
      default: null,
      ":hover": colors.foreground,
    },
    textDecorationLine: "underline",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  authorizedAction: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    color: colors.foreground,
  },
  backIcon: {
    display: "inline-block",
    height: "0.75rem",
    textDecorationLine: "underline",
    width: "0.75rem",
  },
  dialogAction: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
      ":is(.dark *)": "white",
      ":is(.dark *):hover": "rgb(255 255 255 / 0.9)",
    },
    borderRadius: radii.full,
    boxShadow: shadows.sm,
    color: {
      default: colors.primaryForeground,
      ":is(.dark *)": "black",
    },
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  dialogDescription: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    lineHeight: 1.36,
    textAlign: "center",
    width: "100%",
  },
  dialogFooter: {
    display: "grid",
    gap: "0.5rem",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    justifyContent: {
      default: null,
      [media.sm]: "normal",
    },
  },
  dialogHeader: {
    alignItems: "center",
    gap: "0.5rem",
    textAlign: "center",
  },
  dialogTitle: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    fontWeight: 600,
    letterSpacing: "normal",
    lineHeight: "1.25rem",
  },
  disabled: {
    cursor: "not-allowed",
    opacity: 0.5,
  },
  heading: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  permissionAction: {
    height: "2rem",
    width: "2rem",
  },
  permissionCopy: {
    flex: "1",
  },
  permissionRow: {
    display: "flex",
    gap: "1rem",
    paddingBlock: "0.5rem",
  },
  permissionTitle: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.25rem",
  },
  rowWithAction: {
    alignItems: "center",
    justifyContent: "space-between",
  },
  rowWithoutAction: {
    alignItems: "flex-start",
    justifyContent: "flex-start",
  },
  troubleshooting: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  unauthorizedTitle: {
    color: "rgb(239 68 68)",
  },
});
