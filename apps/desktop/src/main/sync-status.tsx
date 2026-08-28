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
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";

import {
  colors,
  radii,
  shadows,
  spacing,
} from "@anlg/design-system/tokens.stylex";
import { getCloudsyncStatus, syncCloudsyncNow } from "@anlg/plugin-db";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { formatDistanceToNow } from "@anlg/utils";

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
const TRANSIENT_FAILURES_BEFORE_WARNING = 3;

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
      case "approval_pending":
        return {
          kind: "connecting" as const,
          label: t`Waiting for device approval`,
          description: t`Open Anarlog on a device that already has access, then approve this device.`,
        };
      case "identity_mismatch":
        return {
          kind: "error" as const,
          label: t`Cloud sync identity mismatch`,
          description: t`This device's sync identity does not match your account. Sign in again or check Sync settings.`,
        };
      case "keychain_access":
        return {
          kind: "error" as const,
          label: t`Sync needs attention`,
          description: t`macOS could not access your recovery key. Repair Keychain access, then resume sync.`,
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

    if (status?.last_error_kind === "auth") {
      return {
        kind: "error" as const,
        label: t`Sign in again`,
        description: t`Sign out and sign in again to resume cloud sync.`,
      };
    }

    if (status?.last_error_kind === "fatal" && status.running === false) {
      return {
        kind: "error" as const,
        label: t`Sync issue`,
        description: t`Cloud sync could not start on this device. Open Sync settings to try again.`,
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

    if (
      status &&
      status.consecutive_failures >= TRANSIENT_FAILURES_BEFORE_WARNING
    ) {
      return {
        kind: "error" as const,
        label: t`Sync issue`,
        description: t`Anarlog will retry automatically. This does not affect your notes.`,
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

  if (deferredForCapture && view.kind === "deferred") {
    return null;
  }

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
          {...stylex.props(styles.indicator)}
        >
          {view.kind === "error" && (
            <CloudWarning {...stylex.props(styles.icon, styles.errorIcon)} />
          )}
          {view.kind === "connecting" && (
            <CircleNotch
              {...stylex.props(styles.icon, styles.spinIcon, styles.syncIcon)}
            />
          )}
          {view.kind === "syncing" && (
            <ArrowsClockwise
              {...stylex.props(styles.icon, styles.spinIcon, styles.syncIcon)}
            />
          )}
          {view.kind === "deferred" && (
            <HardDrive {...stylex.props(styles.icon)} />
          )}
          {view.kind === "paused" && (
            <CloudSlash {...stylex.props(styles.icon)} />
          )}
          {view.kind === "synced" && (
            <CheckCircle {...stylex.props(styles.icon, styles.successIcon)} />
          )}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="top" align="end" sx={styles.menu}>
        <div {...stylex.props(styles.menuHeader)}>
          <p {...stylex.props(styles.menuTitle)}>{view.label}</p>
          {view.description && (
            <p {...stylex.props(styles.menuDescription)}>{view.description}</p>
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
          <ArrowsClockwise {...stylex.props(styles.icon)} />
          {canRetry ? <Trans>Retry</Trans> : <Trans>Sync now</Trans>}
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={setSyncEnabledMutation.isPending}
          onSelect={() => setSyncEnabledMutation.mutate(!syncPreferred)}
        >
          {syncPreferred ? (
            <Pause {...stylex.props(styles.icon)} />
          ) : (
            <Play {...stylex.props(styles.icon)} />
          )}
          {syncPreferred ? (
            <Trans>Pause sync</Trans>
          ) : (
            <Trans>Resume sync</Trans>
          )}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={openSyncSettings}>
          <Gear {...stylex.props(styles.icon)} />
          <Trans>Sync settings</Trans>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  errorIcon: {
    color: "rgb(202 138 4)",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  indicator: {
    alignItems: "center",
    backdropFilter: "blur(8px)",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.background} 90%, transparent)`,
      ":focus-visible": colors.accent,
    },
    borderColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: spacing.sm,
    boxShadow: shadows.sm,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
      ":focus-visible": colors.foreground,
    },
    display: "flex",
    height: "1.75rem",
    justifyContent: "center",
    outlineWidth: "2px",
    outlineStyle: "solid",
    outlineColor: "transparent",
    outlineOffset: "2px",
    position: "absolute",
    right: spacing.sm,
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
    zIndex: 10,
  },
  menu: {
    width: "16rem",
  },
  menuDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.125rem",
    overflowWrap: "break-word",
  },
  menuHeader: {
    paddingBlock: "0.375rem",
    paddingInline: spacing.sm,
  },
  menuTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  spinIcon: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  successIcon: {
    color: "rgb(16 185 129)",
  },
  syncIcon: {
    color: "rgb(59 130 246)",
  },
});

export { styles as syncStatusStyles };
