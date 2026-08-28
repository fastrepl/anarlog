import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CaretDown,
  CheckCircle,
  CircleNotch,
  CloudSlash,
  Desktop,
  DeviceMobile,
  PencilSimple,
  Plugs,
  Plus,
  Shield,
  ShieldCheck,
  Warning,
  Watch,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
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
import { Input } from "@anlg/ui/components/ui/input";
import { Switch } from "@anlg/ui/components/ui/switch";
import { formatDistanceToNow } from "@anlg/utils";

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
  renameSyncDevice,
  requestSyncDevices,
  sealDeviceEnrollment,
  type SyncDeviceKind,
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
  onRename,
}: {
  name: string | null;
  current: boolean;
  onRename?: () => void;
}) {
  const { t } = useLingui();
  return (
    <div {...stylex.props(styles.deviceTitleRow)}>
      <p {...stylex.props(styles.truncatedMedium)}>
        {name || t`Unnamed device`}
      </p>
      {current ? (
        <Badge variant="secondary" size="sm" sx={styles.noShrink}>
          <Trans>This device</Trans>
        </Badge>
      ) : null}
      {current && onRename ? (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          sx={styles.renameButton}
          aria-label={t`Rename device`}
          onClick={onRename}
        >
          <PencilSimple {...stylex.props(styles.smallIcon)} />
        </Button>
      ) : null}
    </div>
  );
}

function isValidDeviceName(name: string) {
  const trimmed = name.trim();
  return trimmed.length > 0 && new TextEncoder().encode(trimmed).length <= 128;
}

function RenameDeviceDialog({
  name,
  pending,
  error,
  onOpenChange,
  onRename,
}: {
  name: string | null;
  pending: boolean;
  error: Error | null;
  onOpenChange: (open: boolean) => void;
  onRename: (name: string) => void;
}) {
  const { t } = useLingui();
  const form = useForm({
    defaultValues: { name: name ?? "" },
    onSubmit: ({ value }) => {
      if (isValidDeviceName(value.name)) {
        onRename(value.name.trim());
      }
    },
  });

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent sx={styles.smallDialog}>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            event.stopPropagation();
            void form.handleSubmit();
          }}
        >
          <DialogHeader>
            <DialogTitle>
              <Trans>Rename this device</Trans>
            </DialogTitle>
            <DialogDescription>
              <Trans>
                This name is synced to your other devices signed in to this
                account.
              </Trans>
            </DialogDescription>
          </DialogHeader>
          <form.Field name="name">
            {(field) => (
              <Input
                autoFocus
                aria-label={t`Device name`}
                sx={styles.dialogInput}
                maxLength={128}
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </form.Field>
          {error ? (
            <p {...stylex.props(styles.errorWithMargin)}>{error.message}</p>
          ) : null}
          <DialogFooter sx={styles.dialogFooter}>
            <Button
              type="button"
              variant="outline"
              disabled={pending}
              onClick={() => onOpenChange(false)}
            >
              <Trans>Cancel</Trans>
            </Button>
            <form.Subscribe selector={(state) => state.values.name}>
              {(value) => (
                <Button
                  type="submit"
                  disabled={pending || !isValidDeviceName(value)}
                >
                  {pending ? (
                    <CircleNotch
                      {...stylex.props(styles.icon, styles.spinning)}
                    />
                  ) : null}
                  <Trans>Save</Trans>
                </Button>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

const DEVICE_KIND_ICONS = {
  desktop: Desktop,
  mobile: DeviceMobile,
  watch: Watch,
} as const;

function resolveDeviceKind(kind: unknown): SyncDeviceKind {
  if (kind === "mobile" || kind === "watch") {
    return kind;
  }
  return "desktop";
}

function DeviceKindIcon({ kind }: { kind?: string | null }) {
  const resolved = resolveDeviceKind(kind);
  const Icon = DEVICE_KIND_ICONS[resolved];
  return (
    <Icon
      aria-hidden="true"
      data-device-kind={resolved}
      {...stylex.props(styles.deviceKindIcon)}
    />
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
      sx={styles.disconnectButton}
      disabled={isPending}
      onClick={() => onDisconnect(fingerprint)}
    >
      {isPending && pendingFingerprint === fingerprint ? (
        <CircleNotch {...stylex.props(styles.smallIcon, styles.spinning)} />
      ) : (
        <Plugs {...stylex.props(styles.smallIcon)} />
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
        return (
          <CheckCircle
            {...stylex.props(styles.smallIcon, styles.successIcon)}
          />
        );
      case "progress":
        return (
          <ArrowsClockwise
            {...stylex.props(styles.smallIcon, styles.progressIcon)}
          />
        );
      case "failed":
        return (
          <Warning {...stylex.props(styles.smallIcon, styles.warningIcon)} />
        );
    }
  })();

  return (
    <li {...stylex.props(styles.logEntry)}>
      <div {...stylex.props(styles.logIcon)}>{icon}</div>
      <div {...stylex.props(styles.logCopy)}>
        <div {...stylex.props(styles.logHeading)}>
          <p {...stylex.props(styles.mediumSmall)}>
            {entry.trigger === "manual" ? t`Manual sync` : t`Background sync`}
          </p>
          <time {...stylex.props(styles.logTime)}>
            {new Date(entry.timestamp_ms).toLocaleTimeString(undefined, {
              hour: "numeric",
              minute: "2-digit",
              second: "2-digit",
            })}
          </time>
        </div>
        <p {...stylex.props(styles.logSummary)}>{summary}</p>
        {entry.error && (
          <p {...stylex.props(styles.logError)}>
            <Trans>
              Anarlog couldn't complete this sync. Your notes are safe on this
              device.
            </Trans>
          </p>
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
  const [renamingDevice, setRenamingDevice] = useState<{
    fingerprint: string;
    name: string | null;
  } | null>(null);
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
  const renameDeviceMutation = useMutation({
    mutationFn: ({
      fingerprint,
      name,
    }: {
      fingerprint: string;
      name: string;
    }) => renameSyncDevice(session!.access_token, fingerprint, name),
    onSuccess: async () => {
      setRenamingDevice(null);
      await queryClient.invalidateQueries({
        queryKey: ["sync-devices", session?.user.id],
      });
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
      <div {...stylex.props(styles.loading)}>
        <CircleNotch
          aria-label={t`Loading sync settings`}
          {...stylex.props(styles.loadingIcon, styles.spinning)}
        />
      </div>
    );
  }

  const openAccountSettings = () => {
    openNew({ type: "settings", state: { tab: "account" } });
  };

  if (!session || !isPro) {
    return (
      <div {...stylex.props(styles.page)}>
        <SettingsPageTitle title={<Trans>Sync</Trans>} />
        <div {...stylex.props(styles.upsell)}>
          <div {...stylex.props(styles.upsellCopy)}>
            <div {...stylex.props(styles.statusIconFrame, styles.mutedFrame)}>
              <CloudSlash {...stylex.props(styles.mutedIcon)} />
            </div>
            <div>
              <h3 {...stylex.props(styles.smallHeading)}>
                {session ? (
                  <Trans>Cloud sync is available with Anarlog Pro</Trans>
                ) : (
                  <Trans>Sign in to use cloud sync</Trans>
                )}
              </h3>
              <p {...stylex.props(styles.description)}>
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
          status.last_error_kind === "auth"
            ? t`Sign out and sign in again to resume cloud sync.`
            : status.last_error_kind === "transient"
              ? t`Anarlog will retry automatically.`
              : t`Anarlog will keep retrying.`,
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
          <ArrowsClockwise
            {...stylex.props(styles.icon, styles.spinning, styles.progressIcon)}
          />
        );
      case "synced":
        return (
          <CheckCircle {...stylex.props(styles.icon, styles.successIcon)} />
        );
      case "error":
        return <Warning {...stylex.props(styles.icon, styles.warningIcon)} />;
      case "paused":
      case "local":
        return <CloudSlash {...stylex.props(styles.mutedIcon)} />;
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
    <div {...stylex.props(styles.page)}>
      <SettingsPageTitle title={<Trans>Sync</Trans>} />

      <section {...stylex.props(styles.syncSection)}>
        <div {...stylex.props(styles.statusRow)}>
          <div {...stylex.props(styles.statusCopy)}>
            <div {...stylex.props(styles.statusIconFrame)}>{statusIcon}</div>
            <div {...stylex.props(styles.minWidth)}>
              <h3 {...stylex.props(styles.smallHeading)}>{statusView.label}</h3>
              <p {...stylex.props(styles.description)}>
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
          <p {...stylex.props(styles.errorSmall)}>{mutationError.message}</p>
        )}

        <div {...stylex.props(styles.syncNowRow)}>
          <p {...stylex.props(styles.mutedSmall)}>
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
              {...stylex.props(
                styles.smallIcon,
                syncNowMutation.isPending && styles.spinning,
              )}
            />
            <Trans>Sync now</Trans>
          </Button>
        </div>

        <div {...stylex.props(styles.logPanel)}>
          <button
            type="button"
            aria-label={syncLogOpen ? t`Hide sync log` : t`View sync log`}
            aria-expanded={syncLogOpen}
            {...stylex.props(styles.logToggle)}
            onClick={() => setSyncLogOpen((open) => !open)}
          >
            <div>
              <h3 {...stylex.props(styles.mediumSmall)}>
                <Trans>Sync log</Trans>
              </h3>
              <p {...stylex.props(styles.logHint)}>
                <Trans>Recent activity from this app session.</Trans>
              </p>
            </div>
            <CaretDown
              {...stylex.props(
                styles.logCaret,
                syncLogOpen && styles.logCaretOpen,
              )}
            />
          </button>

          {syncLogOpen && (
            <div {...stylex.props(styles.logBody)}>
              {status?.activity_log?.length ? (
                <ol {...stylex.props(styles.logList)}>
                  {status.activity_log.map((entry, index) => (
                    <SyncLogEntry
                      key={`${entry.timestamp_ms}-${index}`}
                      entry={entry}
                    />
                  ))}
                </ol>
              ) : (
                <p {...stylex.props(styles.logEmpty)}>
                  <Trans>No sync activity yet.</Trans>
                </p>
              )}
            </div>
          )}
        </div>
      </section>

      {cloudStorageService && (
        <section {...stylex.props(styles.storageWarning)}>
          <div {...stylex.props(styles.warningRow)}>
            <div {...stylex.props(styles.warningIconFrame)}>
              <Warning {...stylex.props(styles.icon, styles.warningIcon)} />
            </div>
            <div {...stylex.props(styles.minWidth)}>
              <h3 {...stylex.props(styles.smallHeading)}>
                <Trans>
                  Your storage location is inside {cloudStorageService}
                </Trans>
              </h3>
              <p {...stylex.props(styles.description)}>
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
                sx={styles.topMarginButton}
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
        <div {...stylex.props(styles.sectionHeader)}>
          <h2 {...stylex.props(styles.sectionHeading)}>
            <Trans>Devices</Trans>
          </h2>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setAddDeviceOpen(true)}
          >
            <Plus {...stylex.props(styles.smallIcon)} />
            <Trans>Add device</Trans>
          </Button>
        </div>
        <div {...stylex.props(styles.deviceList)}>
          {devicesQuery.isPending && (
            <div {...stylex.props(styles.deviceLoading)}>
              <CircleNotch
                aria-label={t`Loading devices`}
                {...stylex.props(styles.mutedIcon, styles.spinning)}
              />
            </div>
          )}
          {devicesQuery.isError && (
            <div {...stylex.props(styles.deviceErrorRow)}>
              <p {...stylex.props(styles.errorSmall)}>
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
                {...stylex.props(styles.deviceRow)}
              >
                <DeviceKindIcon kind={device.deviceKind} />
                <div {...stylex.props(styles.deviceCopy)}>
                  <DeviceTitle
                    name={device.deviceName}
                    current={current}
                    onRename={
                      current
                        ? () => {
                            renameDeviceMutation.reset();
                            setRenamingDevice({
                              fingerprint: device.deviceFingerprint,
                              name: device.deviceName,
                            });
                          }
                        : undefined
                    }
                  />
                  <p
                    {...stylex.props(styles.deviceDetail)}
                  >{t`Last seen ${formatDistanceToNow(new Date(device.lastSeenAt))}`}</p>
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
                            <CircleNotch
                              {...stylex.props(
                                styles.smallIcon,
                                styles.spinning,
                              )}
                            />
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
              <div key={device.requestId} {...stylex.props(styles.deviceRow)}>
                <DeviceKindIcon kind={device.deviceKind} />
                <div {...stylex.props(styles.deviceCopy)}>
                  <DeviceTitle
                    name={device.deviceName}
                    current={current}
                    onRename={
                      current
                        ? () => {
                            renameDeviceMutation.reset();
                            setRenamingDevice({
                              fingerprint: device.deviceFingerprint,
                              name: device.deviceName,
                            });
                          }
                        : undefined
                    }
                  />
                  <p {...stylex.props(styles.deviceDetail)}>
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
                          <CircleNotch
                            {...stylex.props(styles.smallIcon, styles.spinning)}
                          />
                        )}
                      <Trans>Approve</Trans>
                    </Button>
                  )}
                {!current && device.status === "sealed" && (
                  <span {...stylex.props(styles.approved)}>
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
              <p {...stylex.props(styles.noDevices)}>
                <Trans>No devices registered yet.</Trans>
              </p>
            )}
        </div>
        {deviceMutationError && (
          <p {...stylex.props(styles.errorWithMargin)}>
            {deviceMutationError.message}
          </p>
        )}
      </section>

      <section>
        <h2 {...stylex.props(styles.securityHeading)}>
          <Trans>Security</Trans>
        </h2>
        <div {...stylex.props(styles.securityRow)}>
          <div {...stylex.props(styles.statusIconFrame)}>
            {e2eeIdentityQuery.data?.configured ? (
              <ShieldCheck {...stylex.props(styles.icon, styles.successIcon)} />
            ) : (
              <Shield {...stylex.props(styles.mutedIcon)} />
            )}
          </div>
          <div>
            <h3 {...stylex.props(styles.smallHeading)}>
              <Trans>End-to-end encryption</Trans>
            </h3>
            <p {...stylex.props(styles.description)}>
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
                sx={styles.topMarginButton}
                disabled={repairKeychainMutation.isPending}
                onClick={() => repairKeychainMutation.mutate()}
              >
                {repairKeychainMutation.isPending && (
                  <CircleNotch
                    {...stylex.props(styles.smallIcon, styles.spinning)}
                  />
                )}
                <Trans>Repair Keychain Access</Trans>
              </Button>
            )}
            {credentialBlock === "approval_pending" && (
              <Button
                variant="outline"
                size="sm"
                sx={styles.topMarginButton}
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
      {renamingDevice ? (
        <RenameDeviceDialog
          key={renamingDevice.fingerprint}
          name={renamingDevice.name}
          pending={renameDeviceMutation.isPending}
          error={renameDeviceMutation.error}
          onOpenChange={(open) => {
            if (!open && !renameDeviceMutation.isPending) {
              setRenamingDevice(null);
            }
          }}
          onRename={(name) =>
            renameDeviceMutation.mutate({
              fingerprint: renamingDevice.fingerprint,
              name,
            })
          }
        />
      ) : null}
      <Dialog open={addDeviceOpen} onOpenChange={setAddDeviceOpen}>
        <DialogContent sx={styles.smallDialog}>
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
          <p {...stylex.props(styles.dialogDescription)}>
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

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  approved: {
    color: "rgb(16 185 129)",
    fontSize: "0.75rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  deviceCopy: {
    flex: "1",
    minWidth: 0,
  },
  deviceDetail: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
  },
  deviceErrorRow: {
    alignItems: "center",
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  deviceKindIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  deviceList: {
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  deviceLoading: {
    alignItems: "center",
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    justifyContent: "center",
    paddingBlock: "1.25rem",
    paddingInline: "1rem",
  },
  deviceRow: {
    alignItems: "center",
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    gap: "0.75rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  deviceTitleRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  dialogDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
  },
  dialogFooter: {
    marginTop: "1.25rem",
  },
  dialogInput: {
    marginTop: "1rem",
  },
  disconnectButton: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in srgb, ${colors.destructive} 10%, transparent)`,
    },
    borderColor: {
      default: null,
      ":hover": colors.destructive,
    },
    color: {
      default: colors.destructive,
      ":hover": colors.destructive,
    },
  },
  errorSmall: {
    color: "rgb(239 68 68)",
    fontSize: "0.75rem",
  },
  errorWithMargin: {
    color: "rgb(239 68 68)",
    fontSize: "0.75rem",
    marginTop: "0.5rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  loading: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
    minHeight: "12rem",
  },
  loadingIcon: {
    color: colors.mutedForeground,
    height: "1.25rem",
    width: "1.25rem",
  },
  logBody: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  logCaret: {
    color: colors.mutedForeground,
    height: "0.875rem",
    transitionDuration: "150ms",
    transitionProperty: "transform",
    width: "0.875rem",
  },
  logCaretOpen: {
    transform: "rotate(180deg)",
  },
  logCopy: {
    flex: "1",
    minWidth: 0,
  },
  logEmpty: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    paddingBlock: "0.5rem",
    textAlign: "center",
  },
  logEntry: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    gap: "0.75rem",
    paddingBottom: {
      default: "0.75rem",
      ":last-child": 0,
    },
    paddingTop: {
      default: "0.75rem",
      ":first-child": 0,
    },
  },
  logError: {
    color: "rgb(239 68 68)",
    fontSize: "0.75rem",
    marginTop: "0.25rem",
    overflowWrap: "anywhere",
  },
  logHeading: {
    alignItems: "baseline",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
  },
  logHint: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    marginTop: "0.125rem",
  },
  logIcon: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1.25rem",
    justifyContent: "center",
    marginTop: "0.125rem",
    width: "1.25rem",
  },
  logList: {
    maxHeight: "16rem",
    overflowY: "auto",
  },
  logPanel: {
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  logSummary: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.125rem",
  },
  logTime: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.6875rem",
  },
  logToggle: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.muted} 40%, transparent)`,
    },
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    width: "100%",
  },
  mediumSmall: {
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  minWidth: {
    minWidth: 0,
  },
  mutedFrame: {
    backgroundColor: colors.muted,
  },
  mutedIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  mutedSmall: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  noDevices: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    paddingBlock: "1.25rem",
    paddingInline: "1rem",
    textAlign: "center",
  },
  noShrink: {
    flexShrink: 0,
  },
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
  progressIcon: {
    color: "rgb(59 130 246)",
  },
  renameButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    height: "1.5rem",
    width: "1.5rem",
  },
  sectionHeader: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
    marginBottom: "1rem",
  },
  sectionHeading: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
  },
  securityHeading: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
    marginBottom: "1rem",
  },
  securityRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.75rem",
  },
  smallDialog: {
    maxWidth: "24rem",
  },
  smallHeading: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  statusCopy: {
    display: "flex",
    gap: "0.75rem",
    minWidth: 0,
  },
  statusIconFrame: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    justifyContent: "center",
    width: "2.25rem",
  },
  statusRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  storageWarning: {
    backgroundColor: "rgb(245 158 11 / 0.05)",
    borderColor: "rgb(245 158 11 / 0.4)",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1.25rem",
  },
  successIcon: {
    color: "rgb(16 185 129)",
  },
  syncNowRow: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  syncSection: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  topMarginButton: {
    marginTop: "0.75rem",
  },
  truncatedMedium: {
    fontSize: "0.875rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  upsell: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  upsellCopy: {
    display: "flex",
    gap: "0.75rem",
  },
  warningIcon: {
    color: "rgb(245 158 11)",
  },
  warningIconFrame: {
    alignItems: "center",
    backgroundColor: "rgb(245 158 11 / 0.1)",
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    justifyContent: "center",
    width: "2.25rem",
  },
  warningRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.75rem",
  },
});

export { styles as settingsSyncStyles };
