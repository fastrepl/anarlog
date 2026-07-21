import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CloudAlertIcon,
  CloudCheckIcon,
  CloudOffIcon,
  Loader2Icon,
  PauseIcon,
  PlayIcon,
  RefreshCwIcon,
  SettingsIcon,
  SparklesIcon,
} from "lucide-react";
import { useSyncExternalStore } from "react";

import {
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  syncCloudsyncNow,
} from "@hypr/plugin-db";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn, formatDistanceToNow } from "@hypr/utils";

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
  const { isPro, isReady, upgradeToPro } = useBillingAccess();
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
  const statusQuery = useQuery({
    queryKey: STATUS_QUERY_KEY,
    queryFn: getCloudsyncStatus,
    refetchInterval: STATUS_POLL_INTERVAL_MS,
    enabled: Boolean(session) && isPro && syncPreferred,
  });

  const openSyncSettings = () => {
    openNewTab({ type: "settings", state: { tab: "app" } });
  };

  const preferenceMutation = useMutation({
    mutationKey: ["cloudsync-preference"],
    mutationFn: async (enabled: boolean) => {
      if (!session) {
        return;
      }
      if (enabled) {
        const identity = await getE2eeIdentityStatus(session.user.id);
        if (!identity.configured) {
          openSyncSettings();
          return;
        }
      }
      await setSettingValue("cloud_sync_enabled", enabled);
      const result = await applyCloudsyncPreference(session);
      if (result === "account_mismatch") {
        await auth.signOut();
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY });
    },
  });

  const syncNowMutation = useMutation({
    mutationFn: syncCloudsyncNow,
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY });
    },
  });

  if (!session || !isReady || !settingsReady) {
    return null;
  }

  const status = statusQuery.data;
  const view = (() => {
    if (!isPro) {
      return {
        kind: "unavailable" as const,
        label: t`Cloud sync`,
        description: t`Available with Anarlog Pro`,
      };
    }

    if (!syncPreferred) {
      return {
        kind: "paused" as const,
        label: t`Sync paused`,
        description: t`Your notes are stored on this device only`,
      };
    }

    if (credentialBlock === "device_limit") {
      return {
        kind: "error" as const,
        label: t`Device limit reached`,
        description: t`This account already syncs on 5 devices. Remove another device to sync here.`,
      };
    }

    if (
      status &&
      (status.last_error_kind === "auth" ||
        status.last_error_kind === "fatal" ||
        status.consecutive_failures > 0)
    ) {
      return {
        kind: "error" as const,
        label: t`Sync issue`,
        description: status.last_error ?? t`Anarlog will keep retrying`,
      };
    }

    if (!status || !status.configured || !status.running) {
      return {
        kind: "connecting" as const,
        label: t`Connecting...`,
        description: t`Setting up cloud sync`,
      };
    }

    if (status.has_unsent_changes || status.last_sync_at_ms === null) {
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

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t`Cloud sync status: ${view.label}`}
          data-testid="sync-status-indicator"
          className={cn([
            "fixed right-3 bottom-3 z-40",
            "border-border/60 bg-background/85 flex size-7 items-center justify-center rounded-full border shadow-sm backdrop-blur",
            "text-muted-foreground hover:text-foreground transition-colors",
          ])}
        >
          {view.kind === "unavailable" && <CloudOffIcon className="size-4" />}
          {view.kind === "paused" && <CloudOffIcon className="size-4" />}
          {view.kind === "error" && (
            <CloudAlertIcon className="size-4 text-yellow-600" />
          )}
          {view.kind === "connecting" && (
            <Loader2Icon className="size-4 animate-spin" />
          )}
          {view.kind === "syncing" && (
            <RefreshCwIcon className="size-4 animate-spin" />
          )}
          {view.kind === "synced" && <CloudCheckIcon className="size-4" />}
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
        {!isPro ? (
          <DropdownMenuItem onSelect={() => upgradeToPro()}>
            <SparklesIcon className="size-4" />
            <Trans>Upgrade to Pro</Trans>
          </DropdownMenuItem>
        ) : view.kind === "paused" ? (
          <DropdownMenuItem
            disabled={preferenceMutation.isPending}
            onSelect={() => preferenceMutation.mutate(true)}
          >
            <PlayIcon className="size-4" />
            <Trans>Resume sync</Trans>
          </DropdownMenuItem>
        ) : (
          <>
            <DropdownMenuItem
              disabled={syncNowMutation.isPending || view.kind !== "synced"}
              onSelect={() => syncNowMutation.mutate()}
            >
              <RefreshCwIcon className="size-4" />
              <Trans>Sync now</Trans>
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled={preferenceMutation.isPending}
              onSelect={() => preferenceMutation.mutate(false)}
            >
              <PauseIcon className="size-4" />
              <Trans>Pause sync</Trans>
            </DropdownMenuItem>
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={openSyncSettings}>
          <SettingsIcon className="size-4" />
          <Trans>Sync settings</Trans>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
