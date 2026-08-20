import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CaretDown,
  CheckCircle,
  CircleNotch,
  CloudSlash,
  Devices,
  Plugs,
  Plus,
  Shield,
  ShieldCheck,
  Warning,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import {
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  getOrCreateE2eeDeviceIdentity,
  sealE2eeRecoveryKeyForDevice,
  syncCloudsyncNow,
} from "@anlg/plugin-db";
import type { CloudsyncActivityEntry } from "@anlg/plugin-db";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as settingsCommands } from "@anlg/plugin-settings";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { Switch } from "@anlg/ui/components/ui/switch";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { E2eeSetupDialog } from "../general/e2ee-setup";
import { detectCloudStorageService } from "../general/storage/path-utils";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import {
  applyCloudsyncPreference,
  getCloudsyncCredentialBlock,
  refreshCloudsyncForSession,
  subscribeCloudsyncCredentialBlock,
} from "~/auth/cloudsync";
import { getDeviceIdentity } from "~/auth/cloudsync-credentials";
import {
  registerDeviceEnrollment,
  removeSyncDevice,
  requestSyncDevices,
  sealDeviceEnrollment,
} from "~/auth/sync-devices";
import { captureOperationalError } from "~/error-reporting";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  setSettingValue,
  useStoredSettingValuesQuery,
} from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { isKeychainAccessError, repairKeychainAccess } from "~/shared/keychain";
import { useTabs } from "~/store/zustand/tabs";

const STATUS_POLL_INTERVAL_MS = 10_000;
const SYNC_GUIDE_URL = "https://docs.anarlog.so/sync";

async function readE2eeIdentityStatus(accountUserId: string) {
  try {
    return await getE2eeIdentityStatus(accountUserId);
  } catch (error) {
    throw error instanceof Error ? error : new Error(String(error));
  }
}

function DeviceTitle({
  name,
  current,
}: {
  name: string | null;
  current: boolean;
}) {
  const { t } = useLingui();
  return (
    <div className="flex min-w-0 items-center gap-2">
      <p className="truncate text-sm font-medium">
        {name || t`Unnamed device`}
      </p>
      {current ? (
        <Badge variant="secondary" size="sm" className="shrink-0">
          <Trans>This device</Trans>
        </Badge>
      ) : null}
    </div>
  );
}

function DisconnectDeviceButton({
  fingerprint,
  isPending,
  pendingFingerprint,
  onDisconnect,
}: {
  fingerprint: string;
  isPending: boolean;
  pendingFingerprint?: string;
  onDisconnect: (fingerprint: string) => void;
}) {
  return (
    <Button
      variant="outline"
      size="sm"
      className="text-destructive hover:bg-destructive/10 hover:text-destructive"
      disabled={isPending}
      onClick={() => onDisconnect(fingerprint)}
    >
      {isPending && pendingFingerprint === fingerprint ? (
        <CircleNotch className="size-3.5 animate-spin" />
      ) : (
        <Plugs className="size-3.5" />
      )}
      <Trans>Disconnect</Trans>
    </Button>
  );
}

function formatSyncBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function SyncLogEntry({ entry }: { entry: CloudsyncActivityEntry }) {
  const { t } = useLingui();
  const transferSummary = [
    entry.sent_bytes > 0 ? t`Sent ${formatSyncBytes(entry.sent_bytes)}` : null,
    entry.received_bytes > 0
      ? t`Received ${formatSyncBytes(entry.received_bytes)}`
      : null,
  ]
    .filter(Boolean)
    .join(" · ");
  const summary = (() => {
    if (entry.status === "failed") return t`Sync failed`;
    if (transferSummary) return transferSummary;
    if (entry.status === "completed") return t`No changes to sync`;
    return t`Checking for changes`;
  })();
  const icon = (() => {
    switch (entry.status) {
      case "completed":
        return <CheckCircle className="size-3.5 text-emerald-500" />;
      case "progress":
        return <ArrowsClockwise className="size-3.5 text-blue-500" />;
      case "failed":
        return <Warning className="size-3.5 text-amber-500" />;
    }
  })();

  return (
    <li className="flex gap-3 py-3 first:pt-0 last:pb-0">
      <div className="mt-0.5 flex size-5 shrink-0 items-center justify-center">
        {icon}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-3">
          <p className="text-xs font-medium">
            {entry.trigger === "manual" ? t`Manual sync` : t`Background sync`}
          </p>
          <time className="text-muted-foreground shrink-0 text-[11px]">
            {new Date(entry.timestamp_ms).toLocaleTimeString(undefined, {
              hour: "numeric",
              minute: "2-digit",
              second: "2-digit",
            })}
          </time>
        </div>
        <p className="text-muted-foreground mt-0.5 text-xs">{summary}</p>
        {entry.error && (
          <p className="mt-1 text-xs break-words text-red-500">{entry.error}</p>
        )}
      </div>
    </li>
  );
}

export function SettingsSync() {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPro, isReady } = useBillingAccess();
  const openNew = useTabs((state) => state.openNew);
  const queryClient = useQueryClient();
  const [e2eeSetupOpen, setE2eeSetupOpen] = useState(false);
  const [syncLogOpen, setSyncLogOpen] = useState(false);
  const [addDeviceOpen, setAddDeviceOpen] = useState(false);
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
    queryFn: () => readE2eeIdentityStatus(session!.user.id),
    enabled: Boolean(session?.user.id),
    refetchInterval:
      credentialBlock === "approval_pending" ? STATUS_POLL_INTERVAL_MS : false,
    retry: false,
  });
  const devicesQuery = useQuery({
    queryKey: ["sync-devices", session?.user.id],
    queryFn: ({ signal }) => requestSyncDevices(session!.access_token, signal),
    enabled: Boolean(session && isPro),
    refetchInterval: (query) =>
      query.state.data?.pendingDevices.length ? 5_000 : false,
  });
  const deviceIdentityQuery = useQuery({
    queryKey: ["device-identity"],
    queryFn: getDeviceIdentity,
  });
  const removeDeviceMutation = useMutation({
    mutationFn: (fingerprint: string) =>
      removeSyncDevice(session!.access_token, fingerprint),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["sync-devices", session?.user.id],
      });
      if (credentialBlock === "device_limit") {
        const result = await refreshCloudsyncForSession(session!);
        if (result === "account_mismatch") {
          await auth.signOut();
        }
      }
    },
  });
  const approveDeviceMutation = useMutation({
    mutationFn: async ({
      requestId,
      publicKey,
    }: {
      requestId: string;
      publicKey: string;
    }) => {
      const packageValue = await sealE2eeRecoveryKeyForDevice(
        session!.user.id,
        requestId,
        publicKey,
      );
      await sealDeviceEnrollment({
        accessToken: session!.access_token,
        requestId,
        packageValue,
      });
    },
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["sync-devices", session?.user.id],
      }),
  });
  const replaceDeviceMutation = useMutation({
    mutationFn: async (replaceFingerprint: string) => {
      const [device, enrollmentIdentity] = await Promise.all([
        getDeviceIdentity(),
        getOrCreateE2eeDeviceIdentity(session!.user.id),
      ]);
      if (!device.fingerprint) {
        throw new Error(t`Could not identify this device. Try again.`);
      }
      await registerDeviceEnrollment({
        accessToken: session!.access_token,
        publicKey: enrollmentIdentity.publicKey,
        fingerprint: device.fingerprint,
        deviceName: device.name,
        replaceFingerprint,
      });
      const result = await refreshCloudsyncForSession(session!);
      if (result === "account_mismatch") {
        await auth.signOut();
      }
    },
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["sync-devices", session?.user.id],
      }),
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
      return readE2eeIdentityStatus(session.user.id);
    },
    onSuccess: ({ configured }) => {
      if (configured) {
        setSyncEnabledMutation.mutate(true);
      } else {
        setSyncEnabledMutation.mutate(true, {
          onSuccess: () => {
            if (getCloudsyncCredentialBlock() === "setup_required") {
              setE2eeSetupOpen(true);
            }
          },
        });
      }
    },
  });
  const repairKeychainMutation = useMutation({
    mutationKey: ["repair-keychain-access", "cloudsync"],
    mutationFn: repairKeychainAccess,
    onSuccess: async () => {
      const identity = await e2eeIdentityQuery.refetch();
      if (storedSyncEnabled && identity.data?.configured) {
        setSyncEnabledMutation.mutate(true);
      }
    },
  });
  const syncPreferred = setSyncEnabledMutation.isPending
    ? (setSyncEnabledMutation.variables ?? storedSyncEnabled)
    : storedSyncEnabled;
  const syncSwitchChecked =
    syncPreferred && credentialBlock !== "setup_required";
  const statusQuery = useQuery({
    queryKey: statusQueryKey,
    queryFn: getCloudsyncStatus,
    refetchInterval: STATUS_POLL_INTERVAL_MS,
    refetchIntervalInBackground: false,
    enabled: Boolean(session) && isPro && syncPreferred,
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
        <CircleNotch
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
        <div className="flex items-start justify-between gap-4">
          <div className="flex gap-3">
            <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-full">
              <CloudSlash className="text-muted-foreground size-4" />
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
                  Keep notes encrypted and synced across your devices.
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
    if (!syncPreferred) {
      return {
        kind: "paused" as const,
        label: t`Sync paused`,
        description: t`Changes stay on this device until you resume sync.`,
      };
    }
    if (credentialBlock !== null) {
      if (credentialBlock === "approval_pending") {
        return {
          kind: "local" as const,
          label: t`Waiting for device approval`,
          description: t`Open Anarlog on a device that already has access, then approve this device.`,
        };
      }
      if (credentialBlock === "device_limit") {
        return {
          kind: "error" as const,
          label: t`Device limit reached`,
          description: t`Choose a device below to replace, then this device will continue automatically.`,
        };
      }
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
        return (
          <ArrowsClockwise className="size-4 animate-spin text-blue-500" />
        );
      case "synced":
        return <CheckCircle className="size-4 text-emerald-500" />;
      case "error":
        return <Warning className="size-4 text-amber-500" />;
      case "paused":
      case "local":
        return <CloudSlash className="text-muted-foreground size-4" />;
    }
  })();
  const mutationError =
    setSyncEnabledMutation.error ??
    e2eePreflightMutation.error ??
    repairKeychainMutation.error ??
    syncNowMutation.error;
  const deviceMutationError =
    approveDeviceMutation.error ??
    replaceDeviceMutation.error ??
    removeDeviceMutation.error;
  const canRepairKeychainAccess =
    platform() === "macos" &&
    (credentialBlock === "keychain_access" ||
      isKeychainAccessError(e2eeIdentityQuery.error));

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Sync</Trans>} />

      <section className="flex flex-col gap-4">
        <div className="flex items-start justify-between gap-4">
          <div className="flex min-w-0 gap-3">
            <div className="flex size-9 shrink-0 items-center justify-center rounded-full">
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
            checked={syncSwitchChecked}
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
          <p className="text-xs text-red-500">{mutationError.message}</p>
        )}

        <div className="flex items-center justify-between gap-4">
          <p className="text-muted-foreground text-xs">
            <Trans>Keep notes current automatically.</Trans>
          </p>
          <Button
            variant="outline"
            size="sm"
            disabled={
              !syncPreferred ||
              credentialBlock !== null ||
              syncNowMutation.isPending ||
              statusQuery.isFetching ||
              status?.activity_paused === true
            }
            onClick={() => syncNowMutation.mutate()}
          >
            <ArrowsClockwise
              className={cn([
                "size-3.5",
                syncNowMutation.isPending && "animate-spin",
              ])}
            />
            <Trans>Sync now</Trans>
          </Button>
        </div>

        <div className="border-border/60 overflow-hidden rounded-xl border">
          <button
            type="button"
            aria-label={syncLogOpen ? t`Hide sync log` : t`View sync log`}
            aria-expanded={syncLogOpen}
            className="hover:bg-muted/40 flex w-full items-center justify-between gap-4 px-4 py-3 text-left transition-colors"
            onClick={() => setSyncLogOpen((open) => !open)}
          >
            <div>
              <h3 className="text-xs font-medium">
                <Trans>Sync log</Trans>
              </h3>
              <p className="text-muted-foreground mt-0.5 text-[11px]">
                <Trans>Recent activity from this app session.</Trans>
              </p>
            </div>
            <CaretDown
              className={cn([
                "text-muted-foreground size-3.5 transition-transform",
                syncLogOpen && "rotate-180",
              ])}
            />
          </button>

          {syncLogOpen && (
            <div className="border-border/60 border-t px-4 py-3">
              {status?.activity_log?.length ? (
                <ol className="divide-border/60 max-h-64 divide-y overflow-y-auto">
                  {status.activity_log.map((entry, index) => (
                    <SyncLogEntry
                      key={`${entry.timestamp_ms}-${index}`}
                      entry={entry}
                    />
                  ))}
                </ol>
              ) : (
                <p className="text-muted-foreground py-2 text-center text-xs">
                  <Trans>No sync activity yet.</Trans>
                </p>
              )}
            </div>
          )}
        </div>
      </section>

      {cloudStorageService && (
        <section className="rounded-2xl border border-amber-500/40 bg-amber-500/5 p-5">
          <div className="flex items-start gap-3">
            <div className="flex size-9 shrink-0 items-center justify-center rounded-full bg-amber-500/10">
              <Warning className="size-4 text-amber-500" />
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
        <div className="mb-4 flex items-center justify-between gap-4">
          <h2 className="font-sans text-lg font-semibold">
            <Trans>Devices</Trans>
          </h2>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setAddDeviceOpen(true)}
          >
            <Plus className="size-3.5" />
            <Trans>Add device</Trans>
          </Button>
        </div>
        <div className="border-border/60 divide-border/60 divide-y overflow-hidden rounded-xl border">
          {devicesQuery.isPending && (
            <div className="flex items-center justify-center px-4 py-5">
              <CircleNotch
                aria-label={t`Loading devices`}
                className="text-muted-foreground size-4 animate-spin"
              />
            </div>
          )}
          {devicesQuery.isError && (
            <div className="flex items-center justify-between gap-3 px-4 py-3">
              <p className="text-xs text-red-500">
                <Trans>Could not load your devices.</Trans>
              </p>
              <Button
                variant="outline"
                size="sm"
                disabled={devicesQuery.isFetching}
                onClick={() => void devicesQuery.refetch()}
              >
                <Trans>Retry</Trans>
              </Button>
            </div>
          )}
          {devicesQuery.data?.devices.map((device) => {
            const current =
              device.deviceFingerprint ===
              deviceIdentityQuery.data?.fingerprint;
            return (
              <div
                key={device.deviceFingerprint}
                className="flex items-center gap-3 px-4 py-3"
              >
                <Devices className="text-muted-foreground size-4 shrink-0" />
                <div className="min-w-0 flex-1">
                  <DeviceTitle name={device.deviceName} current={current} />
                  <p className="text-muted-foreground text-[11px]">{t`Last seen ${formatDistanceToNow(new Date(device.lastSeenAt))}`}</p>
                </div>
                {!current && (
                  <>
                    {credentialBlock === "device_limit" && (
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={replaceDeviceMutation.isPending}
                        onClick={() =>
                          replaceDeviceMutation.mutate(device.deviceFingerprint)
                        }
                      >
                        {replaceDeviceMutation.isPending &&
                          replaceDeviceMutation.variables ===
                            device.deviceFingerprint && (
                            <CircleNotch className="size-3.5 animate-spin" />
                          )}
                        <Trans>Replace</Trans>
                      </Button>
                    )}
                    {credentialBlock !== "device_limit" && (
                      <DisconnectDeviceButton
                        fingerprint={device.deviceFingerprint}
                        isPending={removeDeviceMutation.isPending}
                        pendingFingerprint={removeDeviceMutation.variables}
                        onDisconnect={removeDeviceMutation.mutate}
                      />
                    )}
                  </>
                )}
              </div>
            );
          })}
          {devicesQuery.data?.pendingDevices.map((device) => {
            const current =
              device.deviceFingerprint ===
              deviceIdentityQuery.data?.fingerprint;
            return (
              <div
                key={device.requestId}
                className="flex items-center gap-3 px-4 py-3"
              >
                <Devices className="text-muted-foreground size-4 shrink-0" />
                <div className="min-w-0 flex-1">
                  <DeviceTitle name={device.deviceName} current={current} />
                  <p className="text-muted-foreground text-[11px]">
                    {device.status === "sealed"
                      ? t`Approved — waiting for this device to finish`
                      : current
                        ? t`Waiting for approval`
                        : t`Approval requested`}
                  </p>
                </div>
                {!current &&
                  device.status === "pending" &&
                  e2eeIdentityQuery.data?.configured &&
                  credentialBlock !== "identity_mismatch" && (
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={approveDeviceMutation.isPending}
                      onClick={() =>
                        approveDeviceMutation.mutate({
                          requestId: device.requestId,
                          publicKey: device.publicKey,
                        })
                      }
                    >
                      {approveDeviceMutation.isPending &&
                        approveDeviceMutation.variables?.requestId ===
                          device.requestId && (
                          <CircleNotch className="size-3.5 animate-spin" />
                        )}
                      <Trans>Approve</Trans>
                    </Button>
                  )}
                {!current && device.status === "sealed" && (
                  <span className="text-xs text-emerald-500">
                    <Trans>Approved</Trans>
                  </span>
                )}
                {!current && (
                  <DisconnectDeviceButton
                    fingerprint={device.deviceFingerprint}
                    isPending={removeDeviceMutation.isPending}
                    pendingFingerprint={removeDeviceMutation.variables}
                    onDisconnect={removeDeviceMutation.mutate}
                  />
                )}
              </div>
            );
          })}
          {!devicesQuery.isPending &&
            !devicesQuery.isError &&
            !devicesQuery.data?.devices.length &&
            !devicesQuery.data?.pendingDevices.length && (
              <p className="text-muted-foreground px-4 py-5 text-center text-xs">
                <Trans>No devices registered yet.</Trans>
              </p>
            )}
        </div>
        {deviceMutationError && (
          <p className="mt-2 text-xs text-red-500">
            {deviceMutationError.message}
          </p>
        )}
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Security</Trans>
        </h2>
        <div className="flex items-start gap-3">
          <div className="flex size-9 shrink-0 items-center justify-center rounded-full">
            {e2eeIdentityQuery.data?.configured ? (
              <ShieldCheck className="size-4 text-emerald-500" />
            ) : (
              <Shield className="text-muted-foreground size-4" />
            )}
          </div>
          <div>
            <h3 className="text-sm font-medium">
              <Trans>End-to-end encryption</Trans>
            </h3>
            <p className="text-muted-foreground mt-1 text-xs leading-5">
              {e2eeIdentityQuery.data?.configured ? (
                <Trans>Keep synced notes readable only on your devices.</Trans>
              ) : credentialBlock === "approval_pending" ? (
                <Trans>
                  This device will start syncing after you approve it from
                  another signed-in device.
                </Trans>
              ) : canRepairKeychainAccess ? (
                <Trans>
                  macOS could not access your recovery key. Repair Keychain
                  access, then resume sync.
                </Trans>
              ) : (
                <Trans>
                  Turn on sync to create or enter your recovery key.
                </Trans>
              )}
            </p>
            {canRepairKeychainAccess && (
              <Button
                variant="outline"
                size="sm"
                className="mt-3"
                disabled={repairKeychainMutation.isPending}
                onClick={() => repairKeychainMutation.mutate()}
              >
                {repairKeychainMutation.isPending && (
                  <CircleNotch className="size-3.5 animate-spin" />
                )}
                <Trans>Repair Keychain Access</Trans>
              </Button>
            )}
            {credentialBlock === "approval_pending" && (
              <Button
                variant="outline"
                size="sm"
                className="mt-3"
                onClick={() => setE2eeSetupOpen(true)}
              >
                <Trans>Use recovery key instead</Trans>
              </Button>
            )}
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
      <Dialog open={addDeviceOpen} onOpenChange={setAddDeviceOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>
              <Trans>Add another device</Trans>
            </DialogTitle>
            <DialogDescription>
              <Trans>
                Install Anarlog and sign in with this account on the new device.
                It will appear here automatically so you can approve it.
              </Trans>
            </DialogDescription>
          </DialogHeader>
          <p className="text-muted-foreground text-xs leading-5">
            <Trans>
              Keep your recovery key saved somewhere safe. You can still use it
              if another approved device is unavailable.
            </Trans>
          </p>
          <DialogFooter>
            <Button onClick={() => setAddDeviceOpen(false)}>
              <Trans>Done</Trans>
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
