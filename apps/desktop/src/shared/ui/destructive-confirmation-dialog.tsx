import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import {
  colors,
  media,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

import { GlassDialogCancelButton, GlassDialogContent } from "./glass-dialog";

export function DestructiveConfirmationDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  pendingLabel,
  isPending = false,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: ReactNode;
  description: ReactNode;
  confirmLabel: ReactNode;
  pendingLabel?: ReactNode;
  isPending?: boolean;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader sx={styles.header}>
          <DialogTitle sx={styles.title}>{title}</DialogTitle>
          <DialogDescription sx={styles.description}>
            {description}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter sx={styles.footer}>
          <GlassDialogCancelButton
            disabled={isPending}
            onClick={() => onOpenChange(false)}
          >
            <Trans>Cancel</Trans>
          </GlassDialogCancelButton>
          <Button
            variant="destructive"
            sx={styles.confirmButton}
            disabled={isPending}
            onClick={onConfirm}
          >
            {isPending ? (pendingLabel ?? confirmLabel) : confirmLabel}
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}

const styles = stylex.create({
  confirmButton: {
    borderRadius: radii.full,
    boxShadow: shadows.sm,
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  description: {
    color: colors.foreground,
    fontSize: "13px",
    lineHeight: 1.36,
    textAlign: "center",
    width: "100%",
  },
  footer: {
    display: "grid",
    gap: "0.5rem",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    justifyContent: {
      default: null,
      [media.sm]: "normal",
    },
  },
  header: {
    alignItems: "center",
    gap: "0.5rem",
    textAlign: {
      default: "center",
      [media.sm]: "center",
    },
  },
  title: {
    color: colors.foreground,
    fontSize: "13px",
    fontWeight: 600,
    letterSpacing: "normal",
    lineHeight: "1.25rem",
  },
});
