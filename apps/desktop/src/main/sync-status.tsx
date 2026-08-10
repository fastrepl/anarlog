import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CheckCircle,
  CircleNotch,
  CloudSlash,
  CloudWarning,
  Gear,
  HardDrive,
  Pause,
  Play,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";

import { getCloudsyncStatus, syncCloudsyncNow } from "@anlg/plugin-db";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import {
  applyCloudsyncPreference,
  getCloudsyncCredentialBlock,
  subscribeCloudsyncCredentialBlock,
} from "~/auth/cloudsync";
import {
  setSettingValue,
  useSettingsReady,
  useStoredSettingValues,
} from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";

const STATUS_QUERY_KEY = ["cloudsync-status-indicator"] as const;
const STATUS_POLL_INTERVAL_MS = 10_000;

export function SyncStatusIndicator() {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPro, isReady } = useBillingAccess();
  const settingsReady = useSettingsReady();
  const storedSettings = useStoredSettingValues();
  const openNewTab = useTabs((state) => state.openNew);
  const queryClient = useQueryClient();

  const session = auth.session;
  const credentialBlock = useSyncExternalStore(
    subscribeCloudsyncCredentialBlock,
    getCloudsyncCredentialBlock,
    getCloudsyncCredentialBlock,
  );
  const syncPreferred = resolveConfigValue(
    "cloud_sync_enabled",
    storedSettings,
  );
  const statusQueryKey = [
    ...STATUS_QUERY_KEY,
    session?.user.id ?? null,
  ] as const;
  const statusQuery = useQuery({
    queryKey: statusQueryKey,
    queryFn: getCloudsyncStatus,
    refetchInterval: STATUS_POLL_INTERVAL_MS,
    refetchIntervalInBackground: true,
    enabled: Boolean(session) && isPro && syncPreferred,
  });

  const openSyncSettings = () => {
    openNewTab({ type: "settings", state: { tab: "sync" } });
  };

  const setSyncEnabledMutation = useMutation({
    mutationKey: ["cloudsync-preference"],
    mutationFn: async (enabled: boolean) => {
      if (!session) {
        return;
      }
      await setSettingValue("cloud_sync_enabled", enabled);
      const result = await applyCloudsyncPreference(session);
      if (result === "account_mismatch") {
        await auth.signOut();
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: statusQueryKey });
    },
  });

  const syncNowMutation = useMutation({
    mutationFn: syncCloudsyncNow,
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: statusQueryKey });
    },
  });

  if (!session || !isReady || !settingsReady || !isPro) {
    return null;
  }

  const status = statusQuery.data;
  const activityPaused = status?.activity_paused === true;
  const deferredForCapture = status?.deferred_for_capture === true;
  const statusUnavailable = credentialBlock === null && statusQuery.isError;
  const view = (() => {
    if (!syncPreferred) {
      return {
        kind: "paused" as const,
        label: t`Sync paused`,
        description: t`Changes stay on this device until you resume sync`,
      };
    }

    switch (credentialBlock) {
      case "device_limit":
        return {
          kind: "error" as const,
          label: t`Device limit reached`,
          description: t`This account already syncs on 5 devices. Remove another device to sync here.`,
        };
      case "identity_mismatch":
        return {
          kind: "error" as const,
          label: t`Cloud sync identity mismatch`,
          description: t`This device's sync identity does not match your account. Sign in again or check Sync settings.`,
        };
      case "not_entitled":
        return {
          kind: "error" as const,
          label: t`Anarlog Pro required`,
          description: t`Anarlog Pro is required to use cloud sync.`,
        };
      case "reauth_required":
        return {
          kind: "error" as const,
          label: t`Sign in again`,
          description: t`Sign out and sign in again to resume cloud sync.`,
        };
      case "setup_required":
        return {
          kind: "error" as const,
          label: t`Cloud sync setup required`,
          description: t`Create or enter your recovery key in Sync settings to start syncing.`,
        };
      case "unavailable":
        return {
          kind: "error" as const,
          label: t`Cloud sync unavailable`,
          description: t`Cloud sync could not start on this device. Open Sync settings to try again.`,
        };
      case null:
        break;
    }

    if (statusUnavailable) {
      return {
        kind: "error" as const,
        label: t`Sync status unavailable`,
        description: t`Anarlog couldn't read cloud sync status. Your notes are still available locally.`,
      };
    }

    if (
      status &&
      (status.last_error_kind === "auth" || status.last_error_kind === "fatal")
    ) {
      return {
        kind: "error" as const,
        label: t`Sync issue`,
        description: status.last_error ?? t`Anarlog will keep retrying`,
      };
    }

    if (deferredForCapture) {
      return {
        kind: "deferred" as const,
        label: t`Saved locally`,
        description: t`Cloud sync resumes after this meeting finishes processing`,
      };
    }

    if (activityPaused) {
      return {
        kind: "deferred" as const,
        label: t`Saved locally`,
        description: t`Cloud sync resumes when the current activity finishes`,
      };
    }

    if (status && status.consecutive_failures > 0) {
      return {
        kind: "error" as const,
        label: t`Sync issue`,
        description:
          status.last_error_kind === "transient"
            ? t`Anarlog will retry automatically. This does not affect your notes.`
            : (status.last_error ?? t`Anarlog will keep retrying`),
      };
    }

    if (status?.recovery_pending && status.recovery_delayed) {
      return {
        kind: "error" as const,
        label: t`Cloud sync delayed`,
        description: t`Anarlog will keep retrying in the background. Your notes remain available locally.`,
      };
    }

    if (status?.recovery_pending) {
      return {
        kind: "syncing" as const,
        label: t`Restoring cloud sync...`,
        description: t`Anarlog is repairing cloud sync in the background. Your notes remain available locally.`,
      };
    }

    if (!status || !status.configured || !status.running) {
      return {
        kind: "connecting" as const,
        label: t`Connecting...`,
        description: t`Setting up cloud sync`,
      };
    }

    if (status.has_unsent_changes === true || status.last_sync_at_ms === null) {
      return {
        kind: "syncing" as const,
        label: t`Syncing...`,
        description: null,
      };
    }

    return {
      kind: "synced" as const,
      label: t`Synced`,
      description: t`Last synced ${formatDistanceToNow(
        new Date(status.last_sync_at_ms),
        { addSuffix: true },
      )}`,
    };
  })();
  const canRetrySync =
    view.kind === "error" && status?.last_error_kind === "transient";
  const canRetryStatus = statusUnavailable;
  const canRetry = canRetrySync || canRetryStatus;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t`Cloud sync status: ${view.label}`}
          data-testid="sync-status-indicator"
          className={cn([
            "fixed right-2 bottom-2 z-40",
            "border-border/60 bg-background/90 flex size-7 items-center justify-center rounded-lg border shadow-sm backdrop-blur",
            "text-muted-foreground hover:text-foreground focus-visible:bg-accent focus-visible:text-foreground outline-hidden transition-colors",
          ])}
        >
          {view.kind === "error" && (
            <CloudWarning className="size-4 text-yellow-600" />
          )}
          {view.kind === "connecting" && (
            <CircleNotch className="size-4 animate-spin text-blue-500" />
          )}
          {view.kind === "syncing" && (
            <ArrowsClockwise className="size-4 animate-spin text-blue-500" />
          )}
          {view.kind === "deferred" && <HardDrive className="size-4" />}
          {view.kind === "paused" && <CloudSlash className="size-4" />}
          {view.kind === "synced" && (
            <CheckCircle className="size-4 text-emerald-500" />
          )}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="top" align="end" className="w-64">
        <div className="px-2 py-1.5">
          <p className="text-sm font-medium">{view.label}</p>
          {view.description && (
            <p className="text-muted-foreground mt-0.5 text-xs break-words">
              {view.description}
            </p>
          )}
        </div>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          disabled={
            !syncPreferred ||
            syncNowMutation.isPending ||
            statusQuery.isFetching ||
            activityPaused ||
            (view.kind !== "synced" && !canRetry)
          }
          onSelect={() => {
            if (canRetryStatus) {
              void statusQuery.refetch();
              return;
            }
            syncNowMutation.mutate();
          }}
        >
          <ArrowsClockwise className="size-4" />
          {canRetry ? <Trans>Retry</Trans> : <Trans>Sync now</Trans>}
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={setSyncEnabledMutation.isPending}
          onSelect={() => setSyncEnabledMutation.mutate(!syncPreferred)}
        >
          {syncPreferred ? (
            <Pause className="size-4" />
          ) : (
            <Play className="size-4" />
          )}
          {syncPreferred ? (
            <Trans>Pause sync</Trans>
          ) : (
            <Trans>Resume sync</Trans>
          )}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={openSyncSettings}>
          <Gear className="size-4" />
          <Trans>Sync settings</Trans>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
