import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangleIcon,
  CheckCircle2Icon,
  CloudOffIcon,
  Loader2Icon,
  RefreshCwIcon,
  ShieldCheckIcon,
  ShieldIcon,
} from "lucide-react";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import {
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  syncCloudsyncNow,
} from "@hypr/plugin-db";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn, formatDistanceToNow } from "@hypr/utils";

import { E2eeSetupDialog } from "../general/e2ee-setup";
import { detectCloudStorageService } from "../general/storage/path-utils";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import {
  applyCloudsyncPreference,
  getCloudsyncCredentialBlock,
  subscribeCloudsyncCredentialBlock,
} from "~/auth/cloudsync";
import { captureOperationalError } from "~/error-reporting";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  setSettingValue,
  useStoredSettingValuesQuery,
} from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";

const STATUS_POLL_INTERVAL_MS = 10_000;
const SYNC_GUIDE_URL = "https://docs.anarlog.so/sync";

export function SettingsSync() {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPro, isReady } = useBillingAccess();
  const openNew = useTabs((state) => state.openNew);
  const queryClient = useQueryClient();
  const [e2eeSetupOpen, setE2eeSetupOpen] = useState(false);
  const lastTrackedSyncAtRef = useRef<number | null>(null);
  const lastTrackedFailureCountRef = useRef<number | null>(null);
  const manualSyncBaselineRef = useRef<number | null>(null);
  const manualFailureBaselineRef = useRef<number | null>(null);
  const manualSyncInFlightRef = useRef(false);
  const manualSyncResultAtRef = useRef<number | null>(null);
  const manualFailureResultRef = useRef<number | null>(null);
  const settingsQuery = useStoredSettingValuesQuery();
  const session = auth.session;
  const credentialBlock = useSyncExternalStore(
    subscribeCloudsyncCredentialBlock,
    getCloudsyncCredentialBlock,
    getCloudsyncCredentialBlock,
  );
  const storedSyncEnabled = settingsQuery.data
    ? resolveConfigValue("cloud_sync_enabled", settingsQuery.data)
    : true;
  const statusQueryKey = [
    "cloudsync-status-settings",
    session?.user.id,
  ] as const;

  const e2eeIdentityQuery = useQuery({
    queryKey: ["e2ee-identity", session?.user.id],
    queryFn: () => getE2eeIdentityStatus(session!.user.id),
    enabled: Boolean(session?.user.id),
  });
  const vaultBaseQuery = useQuery({
    queryKey: ["vault-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const cloudStorageService = vaultBaseQuery.data
    ? detectCloudStorageService(vaultBaseQuery.data)
    : null;
  const setSyncEnabledMutation = useMutation({
    mutationKey: ["cloudsync-preference"],
    mutationFn: async (enabled: boolean) => {
      await setSettingValue("cloud_sync_enabled", enabled);
      const result = await applyCloudsyncPreference(session);
      if (result === "account_mismatch") {
        await auth.signOut();
      }
      return result;
    },
    onSuccess: (result, enabled) => {
      if (result === "account_mismatch") {
        trackAnalyticsEvent("cloud_sync_failed", {
          trigger: enabled ? "enable" : "disable",
          failure_stage: "preference",
        });
        return;
      }
      trackAnalyticsEvent(
        enabled ? "cloud_sync_enabled" : "cloud_sync_disabled",
        {
          entry_point: "settings",
        },
      );
    },
    onError: (error, enabled) => {
      captureOperationalError(error, {
        operation: "cloud_sync_preference_update",
        context: { enabled },
      });
      trackAnalyticsEvent("cloud_sync_failed", {
        trigger: enabled ? "enable" : "disable",
        failure_stage: "preference",
      });
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: statusQueryKey });
    },
  });
  const e2eePreflightMutation = useMutation({
    mutationKey: ["e2ee-preflight"],
    mutationFn: async () => {
      if (!session?.user.id) {
        throw new Error(t`Sign in before enabling encrypted cloud sync`);
      }
      return getE2eeIdentityStatus(session.user.id);
    },
    onSuccess: ({ configured }) => {
      if (configured) {
        setSyncEnabledMutation.mutate(true);
      } else {
        setE2eeSetupOpen(true);
      }
    },
  });
  const syncEnabled = setSyncEnabledMutation.isPending
    ? (setSyncEnabledMutation.variables ?? storedSyncEnabled)
    : storedSyncEnabled && e2eeIdentityQuery.data?.configured !== false;
  const statusQuery = useQuery({
    queryKey: statusQueryKey,
    queryFn: getCloudsyncStatus,
    refetchInterval: STATUS_POLL_INTERVAL_MS,
    refetchIntervalInBackground: true,
    enabled: Boolean(session) && isPro && syncEnabled,
  });
  const syncNowMutation = useMutation({
    mutationFn: syncCloudsyncNow,
    onMutate: () => {
      manualSyncInFlightRef.current = true;
      manualSyncBaselineRef.current = lastTrackedSyncAtRef.current;
      manualFailureBaselineRef.current = lastTrackedFailureCountRef.current;
    },
    onSuccess: () => {
      trackAnalyticsEvent("cloud_sync_completed", {
        trigger: "manual",
      });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "cloud_sync_manual",
      });
      trackAnalyticsEvent("cloud_sync_failed", {
        trigger: "manual",
        failure_stage: "sync",
      });
    },
    onSettled: async (_data, error) => {
      await queryClient.invalidateQueries({ queryKey: statusQueryKey });
      const refreshedStatus =
        queryClient.getQueryData<
          Awaited<ReturnType<typeof getCloudsyncStatus>>
        >(statusQueryKey);
      if (error) {
        manualFailureResultRef.current =
          refreshedStatus?.consecutive_failures ?? null;
      } else {
        manualSyncResultAtRef.current =
          refreshedStatus?.last_sync_at_ms ?? null;
      }
      manualSyncInFlightRef.current = false;
      manualSyncBaselineRef.current = null;
      manualFailureBaselineRef.current = null;
    },
  });
  const status = statusQuery.data;

  useEffect(() => {
    if (!status) return;

    if (
      status.last_sync_at_ms !== null &&
      status.last_sync_at_ms !== lastTrackedSyncAtRef.current
    ) {
      if (lastTrackedSyncAtRef.current !== null) {
        if (
          (manualSyncInFlightRef.current &&
            manualSyncBaselineRef.current === lastTrackedSyncAtRef.current) ||
          manualSyncResultAtRef.current === status.last_sync_at_ms
        ) {
          manualSyncBaselineRef.current = null;
          manualSyncResultAtRef.current = null;
        } else {
          trackAnalyticsEvent("cloud_sync_completed", {
            trigger: "background",
          });
        }
      }
      lastTrackedSyncAtRef.current = status.last_sync_at_ms;
    }

    if (
      lastTrackedFailureCountRef.current !== null &&
      status.consecutive_failures > lastTrackedFailureCountRef.current
    ) {
      if (
        (manualSyncInFlightRef.current &&
          manualFailureBaselineRef.current ===
            lastTrackedFailureCountRef.current) ||
        manualFailureResultRef.current === status.consecutive_failures
      ) {
        manualFailureBaselineRef.current = null;
        manualFailureResultRef.current = null;
      } else {
        trackAnalyticsEvent("cloud_sync_failed", {
          trigger: "background",
          failure_kind: status.last_error_kind ?? "unknown",
        });
      }
    }
    lastTrackedFailureCountRef.current = status.consecutive_failures;
  }, [status]);

  if (settingsQuery.error) {
    throw settingsQuery.error;
  }
  if (settingsQuery.isLoading || !settingsQuery.data || !isReady) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <Loader2Icon
          aria-label={t`Loading sync settings`}
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  const openAccountSettings = () => {
    openNew({ type: "settings", state: { tab: "account" } });
  };

  if (!session || !isPro) {
    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title={<Trans>Sync</Trans>} />
        <div className="border-border/70 bg-card/60 flex items-start justify-between gap-4 rounded-2xl border p-5">
          <div className="flex gap-3">
            <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-full">
              <CloudOffIcon className="text-muted-foreground size-4" />
            </div>
            <div>
              <h3 className="text-sm font-medium">
                {session ? (
                  <Trans>Cloud sync is available with Anarlog Pro</Trans>
                ) : (
                  <Trans>Sign in to use cloud sync</Trans>
                )}
              </h3>
              <p className="text-muted-foreground mt-1 text-xs leading-5">
                <Trans>
                  Keep notes end-to-end encrypted and up to date across your
                  signed-in devices.
                </Trans>
              </p>
            </div>
          </div>
          <Button variant="outline" size="sm" onClick={openAccountSettings}>
            {session ? <Trans>View plans</Trans> : <Trans>Sign in</Trans>}
          </Button>
        </div>
      </div>
    );
  }

  const statusView = (() => {
    if (!syncEnabled) {
      return {
        kind: "paused" as const,
        label: t`Sync paused`,
        description: t`Changes stay on this device until you resume sync.`,
      };
    }
    if (credentialBlock !== null) {
      return {
        kind: "error" as const,
        label: t`Sync needs attention`,
        description:
          credentialBlock === "setup_required"
            ? t`Set up your recovery key to start encrypted cloud sync.`
            : t`Anarlog could not start cloud sync on this device.`,
      };
    }
    if (statusQuery.isError) {
      return {
        kind: "error" as const,
        label: t`Sync status unavailable`,
        description: t`Your notes remain available locally. Try again in a moment.`,
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
        label: t`Sync needs attention`,
        description:
          status.last_error_kind === "transient"
            ? t`Anarlog will retry automatically.`
            : (status.last_error ?? t`Anarlog will keep retrying.`),
      };
    }
    if (status?.activity_paused) {
      return {
        kind: "local" as const,
        label: t`Saved locally`,
        description: status.deferred_for_capture
          ? t`Cloud sync resumes after this meeting finishes processing.`
          : t`Cloud sync resumes when the current activity finishes.`,
      };
    }
    if (status?.recovery_pending) {
      return {
        kind: status.recovery_delayed
          ? ("error" as const)
          : ("syncing" as const),
        label: status.recovery_delayed
          ? t`Cloud sync delayed`
          : t`Restoring cloud sync...`,
        description: t`Your notes remain available locally.`,
      };
    }
    if (!status || !status.configured || !status.running) {
      return {
        kind: "syncing" as const,
        label: t`Connecting...`,
        description: t`Setting up encrypted cloud sync.`,
      };
    }
    if (
      syncNowMutation.isPending ||
      status.has_unsent_changes === true ||
      status.last_sync_at_ms === null
    ) {
      return {
        kind: "syncing" as const,
        label: t`Syncing...`,
        description: t`Sending and receiving your latest changes.`,
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
  const statusIcon = (() => {
    switch (statusView.kind) {
      case "syncing":
        return <RefreshCwIcon className="size-4 animate-spin text-blue-500" />;
      case "synced":
        return <CheckCircle2Icon className="size-4 text-emerald-500" />;
      case "error":
        return <AlertTriangleIcon className="size-4 text-amber-500" />;
      case "paused":
      case "local":
        return <CloudOffIcon className="text-muted-foreground size-4" />;
    }
  })();
  const mutationError =
    setSyncEnabledMutation.error ??
    e2eePreflightMutation.error ??
    syncNowMutation.error;

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Sync</Trans>} />

      <section className="border-border/70 bg-card/60 rounded-2xl border p-5">
        <div className="flex items-start justify-between gap-4">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-full">
              {statusIcon}
            </div>
            <div className="min-w-0">
              <h3 className="text-sm font-medium">{statusView.label}</h3>
              <p className="text-muted-foreground mt-1 text-xs leading-5">
                {statusView.description}
              </p>
            </div>
          </div>
          <Switch
            aria-label={t`Cloud sync`}
            checked={syncEnabled}
            disabled={
              setSyncEnabledMutation.isPending ||
              e2eePreflightMutation.isPending ||
              e2eeIdentityQuery.isLoading
            }
            onCheckedChange={(enabled) => {
              if (enabled) {
                e2eePreflightMutation.mutate();
              } else {
                setSyncEnabledMutation.mutate(false);
              }
            }}
          />
        </div>

        {mutationError && (
          <p className="mt-4 text-xs text-red-500">{mutationError.message}</p>
        )}

        <div className="border-border/60 mt-5 flex items-center justify-between border-t pt-4">
          <p className="text-muted-foreground text-xs">
            <Trans>Sync runs automatically in the background.</Trans>
          </p>
          <Button
            variant="outline"
            size="sm"
            disabled={
              !syncEnabled ||
              syncNowMutation.isPending ||
              statusQuery.isFetching ||
              status?.activity_paused === true
            }
            onClick={() => syncNowMutation.mutate()}
          >
            <RefreshCwIcon
              className={cn([
                "size-3.5",
                syncNowMutation.isPending && "animate-spin",
              ])}
            />
            <Trans>Sync now</Trans>
          </Button>
        </div>
      </section>

      {cloudStorageService && (
        <section className="rounded-2xl border border-amber-500/40 bg-amber-500/5 p-5">
          <div className="flex items-start gap-3">
            <div className="flex size-9 shrink-0 items-center justify-center rounded-full bg-amber-500/10">
              <AlertTriangleIcon className="size-4 text-amber-500" />
            </div>
            <div className="min-w-0">
              <h3 className="text-sm font-medium">
                <Trans>
                  Your storage location is inside {cloudStorageService}
                </Trans>
              </h3>
              <p className="text-muted-foreground mt-1 text-xs leading-5">
                <Trans>
                  Cloud sync and {cloudStorageService} can both change the same
                  files, which can create conflicted copies and incomplete
                  recordings. Move your Anarlog storage location to a folder
                  that {cloudStorageService} does not sync.
                </Trans>
              </p>
              <Button
                variant="outline"
                size="sm"
                className="mt-3"
                onClick={() =>
                  void openerCommands.openUrl(SYNC_GUIDE_URL, null)
                }
              >
                <Trans>Learn more</Trans>
              </Button>
            </div>
          </div>
        </section>
      )}

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Security</Trans>
        </h2>
        <div className="flex items-start gap-3">
          <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-full">
            {e2eeIdentityQuery.data?.configured ? (
              <ShieldCheckIcon className="size-4 text-emerald-500" />
            ) : (
              <ShieldIcon className="text-muted-foreground size-4" />
            )}
          </div>
          <div>
            <h3 className="text-sm font-medium">
              <Trans>End-to-end encryption</Trans>
            </h3>
            <p className="text-muted-foreground mt-1 text-xs leading-5">
              {e2eeIdentityQuery.data?.configured ? (
                <Trans>
                  Your recovery key encrypts synced notes before they leave this
                  device. Anarlog cannot read them.
                </Trans>
              ) : (
                <Trans>
                  Turn on sync to create or enter your recovery key.
                </Trans>
              )}
            </p>
          </div>
        </div>
      </section>

      <E2eeSetupDialog
        open={e2eeSetupOpen}
        onOpenChange={setE2eeSetupOpen}
        accountUserId={session.user.id}
        accessToken={session.access_token}
        onReady={() => {
          setE2eeSetupOpen(false);
          void e2eeIdentityQuery.refetch();
          setSyncEnabledMutation.mutate(true);
        }}
      />
    </div>
  );
}
