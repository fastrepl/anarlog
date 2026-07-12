import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  Loader2Icon,
  Trash2Icon,
  TriangleAlertIcon,
} from "lucide-react";
import { useState } from "react";

import { cleanupLegacyFiles, getLegacyCleanupStatus } from "@hypr/plugin-db";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";

const QUERY_KEY = ["legacy-migration-cleanup"] as const;

export function LegacyMigrationCleanupRow() {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [confirmationOpen, setConfirmationOpen] = useState(false);
  const statusQuery = useQuery({
    queryKey: QUERY_KEY,
    queryFn: getLegacyCleanupStatus,
  });
  const cleanupMutation = useMutation({
    mutationFn: cleanupLegacyFiles,
    onSuccess: async () => {
      setConfirmationOpen(false);
      await queryClient.invalidateQueries({ queryKey: QUERY_KEY });
    },
  });

  const status = statusQuery.data;
  const statusCopy = (() => {
    if (statusQuery.isLoading) {
      return {
        state: "loading" as const,
        label: t`Checking migration...`,
        description: t`Verifying the SQLite migration status`,
      };
    }

    if (statusQuery.error || !status) {
      return {
        state: "warning" as const,
        label: t`Migration status unavailable`,
        description: t`Anarlog could not verify the migration status`,
      };
    }

    if (!status.migrationVerified) {
      return {
        state: "warning" as const,
        label: t`Migration needs attention`,
        description:
          status.blockingReason ??
          t`SQLite migration verification is incomplete`,
      };
    }

    if (status.alreadyCleaned) {
      return {
        state: "success" as const,
        label: t`Migration complete`,
        description: t`Legacy JSON and Markdown files were removed`,
      };
    }

    if (status.available) {
      return {
        state: "success" as const,
        label: t`Migration complete`,
        description: t`${status.fileCount} verified legacy files can be removed`,
      };
    }

    return {
      state: "success" as const,
      label: t`Migration complete`,
      description: t`No legacy JSON or Markdown files remain`,
    };
  })();

  return (
    <>
      <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
        <div className="flex min-w-0 items-center gap-1.5 text-sm">
          {statusCopy.state === "loading" && (
            <Loader2Icon className="text-muted-foreground size-4 shrink-0 animate-spin" />
          )}
          {statusCopy.state === "success" && (
            <CheckCircle2Icon className="size-4 shrink-0 text-green-600" />
          )}
          {statusCopy.state === "warning" && (
            <TriangleAlertIcon className="size-4 shrink-0 text-yellow-600" />
          )}
          <span className="shrink-0 font-medium">{statusCopy.label}</span>
          <span className="text-muted-foreground" aria-hidden="true">
            ·
          </span>
          <span className="text-muted-foreground truncate">
            {statusCopy.description}
          </span>
        </div>
        {status?.migrationVerified && status.available && (
          <Button
            variant="destructive"
            className="h-9 w-full justify-center"
            onClick={() => setConfirmationOpen(true)}
          >
            <Trash2Icon className="size-4" aria-hidden="true" />
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
          <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
            <DialogHeader className="items-center gap-2 px-5 pt-6 text-center sm:text-center">
              <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
                <Trans>Clean up legacy files?</Trans>
              </DialogTitle>
              <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
                <Trans>
                  This will remove {status.fileCount} legacy files and free{" "}
                  {formatBytes(status.totalBytes)}. Your app data will not be
                  affected because the migration to SQLite is complete.
                </Trans>
              </DialogDescription>
            </DialogHeader>

            {cleanupMutation.error && (
              <p className="mx-4 mt-3 text-center text-xs text-red-500">
                {cleanupMutation.error.message}
              </p>
            )}

            <DialogFooter className="grid grid-cols-2 gap-2 px-4 pt-4 pb-4 sm:grid-cols-2 sm:justify-normal">
              <Button
                variant="ghost"
                className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-4 text-xs font-medium shadow-none"
                onClick={() => setConfirmationOpen(false)}
                disabled={cleanupMutation.isPending}
              >
                <Trans>Cancel</Trans>
              </Button>
              <Button
                variant="destructive"
                className="h-8 rounded-full px-4 text-xs font-medium shadow-sm"
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
