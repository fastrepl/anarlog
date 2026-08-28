import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowCounterClockwise,
  CheckCircle,
  CircleNotch,
  Info,
  Trash,
  Warning,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import {
  cleanupLegacyFiles,
  getLegacyCleanupStatus,
  getLegacyImportReport,
  runLegacyImport,
} from "@anlg/plugin-db";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

const QUERY_KEY = ["legacy-migration"] as const;

export function useLegacyMigrationCleanup() {
  const query = useQuery({
    queryKey: QUERY_KEY,
    queryFn: async () => {
      const [status, report] = await Promise.all([
        getLegacyCleanupStatus(),
        getLegacyImportReport(),
      ]);
      return { status, report };
    },
    // The status commands can fail transiently while the database is busy
    // (e.g. during CloudSync enablement); keep retrying instead of parking
    // the row in an error state.
    refetchInterval: (q) => (q.state.status === "error" ? 15_000 : false),
  });

  const status = query.data?.status;
  // A verified migration with no legacy files left needs no user action, so
  // the row (and its section) stays hidden instead of reporting a no-op.
  const visible = status
    ? !status.migrationVerified || status.available
    : !query.isPending;

  return { ...query, visible };
}

export function LegacyMigrationCleanupRow() {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [confirmationOpen, setConfirmationOpen] = useState(false);
  const migrationQuery = useLegacyMigrationCleanup();
  const cleanupMutation = useMutation({
    mutationFn: cleanupLegacyFiles,
    onSuccess: async () => {
      setConfirmationOpen(false);
      await queryClient.invalidateQueries({ queryKey: QUERY_KEY });
    },
  });
  const retryMutation = useMutation({
    mutationFn: () => runLegacyImport(false),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEY });
    },
  });

  const status = migrationQuery.data?.status;
  const report = migrationQuery.data?.report;
  const migrationIssue = (() => {
    if (retryMutation.error) return retryMutation.error.message;

    const latestRun = report?.latestRun;
    if (!latestRun) return status?.blockingReason ?? null;
    if (latestRun.error.trim()) return latestRun.error;

    const issueItem = report.items.find(
      (item) => item.status === "error" || item.status === "partial",
    );
    if (issueItem?.error.trim()) {
      return `${issueItem.sourcePath}: ${issueItem.error}`;
    }
    if (issueItem) {
      return `${issueItem.sourcePath}: ${issueItem.status}`;
    }

    return status?.blockingReason ?? null;
  })();
  const statusCopy = (() => {
    if (status) {
      if (status.migrationReady && !status.migrationVerified) {
        return {
          state: "success" as const,
          label: t`Migration complete`,
          description: t`Older files that differ from your current data were kept as recovery copies. No action is required.`,
        };
      }

      if (!status.migrationVerified) {
        return {
          state: "warning" as const,
          label: t`Migration needs attention`,
          description:
            migrationIssue ?? t`SQLite migration verification is incomplete`,
        };
      }

      return {
        state: "success" as const,
        label: t`Migration complete`,
        description: null,
      };
    }

    if (migrationQuery.isPending && !migrationQuery.error) {
      return {
        state: "loading" as const,
        label: t`Checking migration...`,
        description: t`Verifying the SQLite migration status`,
      };
    }

    return {
      state: "unavailable" as const,
      label: t`Migration status unavailable`,
      description: t`Anarlog will retry automatically. This does not affect your notes.`,
    };
  })();

  if (!migrationQuery.visible) return null;

  return (
    <>
      <div {...stylex.props(styles.row)}>
        <div {...stylex.props(styles.status)}>
          {statusCopy.state === "loading" && (
            <CircleNotch {...stylex.props(styles.loadingIcon)} />
          )}
          {statusCopy.state === "success" && (
            <CheckCircle {...stylex.props(styles.successIcon)} />
          )}
          {statusCopy.state === "warning" && (
            <Warning {...stylex.props(styles.warningIcon)} />
          )}
          {statusCopy.state === "unavailable" && (
            <Info {...stylex.props(styles.infoIcon)} />
          )}
          <div {...stylex.props(styles.statusCopy)}>
            <p {...stylex.props(styles.statusLabel)}>{statusCopy.label}</p>
            {statusCopy.description && (
              <p {...stylex.props(styles.statusDescription)}>
                {statusCopy.description}
              </p>
            )}
          </div>
        </div>
        {status && !status.migrationReady && (
          <Button
            variant="outline"
            sx={styles.actionButton}
            onClick={() => retryMutation.mutate()}
            disabled={retryMutation.isPending}
          >
            {retryMutation.isPending ? (
              <CircleNotch
                {...stylex.props(styles.spinner)}
                aria-hidden="true"
              />
            ) : (
              <ArrowCounterClockwise
                {...stylex.props(styles.icon)}
                aria-hidden="true"
              />
            )}
            {t`Retry`}
          </Button>
        )}
        {status?.migrationVerified && status.available && (
          <Button
            variant="destructive"
            sx={styles.actionButton}
            onClick={() => setConfirmationOpen(true)}
          >
            <Trash {...stylex.props(styles.icon)} aria-hidden="true" />
            <Trans>Clean Up</Trans>
          </Button>
        )}
      </div>

      {status && (
        <Dialog
          open={confirmationOpen}
          onOpenChange={(open) => {
            if (!cleanupMutation.isPending) setConfirmationOpen(open);
          }}
        >
          <DialogContent
            ref={(node) => {
              const closeButton = node?.querySelector<HTMLElement>(
                ":scope > button:last-child",
              );
              if (closeButton) closeButton.hidden = true;
            }}
            sx={styles.dialog}
          >
            <DialogHeader sx={styles.dialogHeader}>
              <DialogTitle sx={styles.dialogTitle}>
                <Trans>Clean up legacy files?</Trans>
              </DialogTitle>
              <DialogDescription sx={styles.dialogDescription}>
                <Trans>
                  This will remove {status.fileCount} legacy files and free{" "}
                  {formatBytes(status.totalBytes)}. Your app data will not be
                  affected because the migration to SQLite is complete.
                </Trans>
              </DialogDescription>
            </DialogHeader>

            {cleanupMutation.error && (
              <p {...stylex.props(styles.dialogError)}>
                {cleanupMutation.error.message}
              </p>
            )}

            <DialogFooter sx={styles.dialogFooter}>
              <Button
                variant="ghost"
                sx={styles.cancelButton}
                onClick={() => setConfirmationOpen(false)}
                disabled={cleanupMutation.isPending}
              >
                <Trans>Cancel</Trans>
              </Button>
              <Button
                variant="destructive"
                sx={styles.confirmButton}
                onClick={() => cleanupMutation.mutate()}
                disabled={cleanupMutation.isPending}
              >
                {cleanupMutation.isPending ? t`Cleaning up...` : t`Clean Up`}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  actionButton: {
    height: "2.25rem",
    justifyContent: "center",
    width: "100%",
  },
  cancelButton: {
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.accent} 80%, transparent)`,
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    boxShadow: "none",
    color: colors.foreground,
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  confirmButton: {
    borderRadius: radii.full,
    boxShadow: shadows.sm,
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  dialog: {
    backdropFilter: "blur(24px)",
    backgroundColor: `color-mix(in oklab, ${colors.card} 95%, transparent)`,
    borderColor: `color-mix(in oklab, ${colors.border} 45%, transparent)`,
    borderRadius: "26px",
    boxShadow: "0 24px 70px rgb(0 0 0 / 0.32)",
    gap: 0,
    maxWidth: "320px",
    overflow: "hidden",
    padding: 0,
    width: "calc(100vw - 48px)",
  },
  dialogDescription: {
    color: colors.foreground,
    fontSize: "13px",
    lineHeight: 1.36,
    textAlign: "center",
    width: "100%",
  },
  dialogError: {
    color: "rgb(239 68 68)",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginInline: "1rem",
    marginTop: "0.75rem",
    textAlign: "center",
  },
  dialogFooter: {
    display: "grid",
    gap: "0.5rem",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    justifyContent: "normal",
    paddingBottom: "1rem",
    paddingInline: "1rem",
    paddingTop: "1rem",
  },
  dialogHeader: {
    alignItems: "center",
    gap: "0.5rem",
    paddingInline: "1.25rem",
    paddingTop: "1.5rem",
    textAlign: "center",
  },
  dialogTitle: {
    color: colors.foreground,
    fontSize: "13px",
    fontWeight: 600,
    letterSpacing: "0em",
    lineHeight: "1.25rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  infoIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
  loadingIcon: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
  row: {
    alignItems: "center",
    display: "grid",
    gap: "0.75rem",
    gridTemplateColumns: "minmax(0, 1fr) 9rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  status: {
    alignItems: "flex-start",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.375rem",
    lineHeight: "1.25rem",
    minWidth: 0,
  },
  statusCopy: {
    minWidth: 0,
  },
  statusDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.125rem",
    overflowWrap: "break-word",
  },
  statusLabel: {
    fontWeight: 500,
  },
  successIcon: {
    color: "rgb(22 163 74)",
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
  warningIcon: {
    color: "rgb(202 138 4)",
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
});
