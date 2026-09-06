import { Button, FieldGroup, Text } from "@expo/ui";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import { useSyncExternalStore } from "react";

import { useAuth } from "@/auth/context";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";
import { formatStorageBytes, useRecordingStorage } from "@/settings/storage";
import { requestSyncDeviceList } from "@/settings/sync-devices";
import {
  getMobileSyncSnapshot,
  retryMobileSync,
  subscribeMobileSync,
  syncMobileNow,
} from "@/sync/mobile-sync";
import { syncStatusPresentation } from "@/sync/status-presentation";

export default function SyncSettings() {
  const auth = useAuth();
  const router = useRouter();
  const snapshot = useSyncExternalStore(
    subscribeMobileSync,
    getMobileSyncSnapshot,
    getMobileSyncSnapshot,
  );
  const presentation = syncStatusPresentation(snapshot);
  const storage = useRecordingStorage();
  const data = storage.data?.[0];
  const sync = useMutation({ mutationFn: syncMobileNow });
  const refresh = useMutation({ mutationFn: auth.refreshBilling });
  const devices = useQuery({
    queryKey: ["sync-devices", auth.session?.user.id],
    queryFn: ({ signal }) =>
      requestSyncDeviceList(auth.session!.access_token, signal),
    enabled: Boolean(auth.session) && !auth.bypass && auth.billing.isPro,
  });
  return (
    <SettingsPage title="Sync & storage">
      <FieldGroup.Section>
        <SettingsRow
          title={
            !auth.billing.isPro
              ? "Saved on this device"
              : presentation.healthy
                ? "Up to date"
                : presentation.title
          }
          description={
            !auth.billing.isPro
              ? "Cloud sync is available during your Pro trial and with a Pro subscription. Your local notes and recordings are still available."
              : presentation.description
          }
        />
        {!auth.bypass && !auth.billing.isPro && (
          <SettingsRow
            title="Explore Anarlog Pro"
            onPress={() => router.push("/settings/pro")}
          />
        )}
        {auth.billing.isPro && presentation.detail && (
          <Text>{presentation.detail}</Text>
        )}
        {auth.billing.isPro && snapshot.phase === "ready" && (
          <Button
            label={
              sync.isPending || snapshot.syncingNow ? "Syncing…" : "Sync now"
            }
            disabled={sync.isPending || snapshot.syncingNow}
            onPress={() => sync.mutate()}
          />
        )}
        {snapshot.phase === "not_entitled" && (
          <Button
            label="Refresh plan"
            disabled={refresh.isPending}
            onPress={() => refresh.mutate()}
          />
        )}
        {auth.billing.isPro &&
          [
            "error",
            "device_limit",
            "identity_mismatch",
            "approval_pending",
          ].includes(snapshot.phase) && (
            <Button label="Try again" onPress={retryMobileSync} />
          )}
        {snapshot.phase === "reauth_required" && (
          <SettingsRow
            title="Sign in again"
            onPress={() => router.push("/settings/account")}
          />
        )}
        <SettingsError error={sync.error || refresh.error} />
      </FieldGroup.Section>
      <FieldGroup.Section title="Recordings">
        <SettingsRow
          title="On this device"
          value={
            data
              ? `${data.local_count} · ${formatStorageBytes(data.local_bytes)}`
              : "Loading…"
          }
        />
        <SettingsRow
          title={auth.billing.isPro ? "Waiting to back up" : "Not backed up"}
          value={data ? String(data.pending_count) : "Loading…"}
        />
        <SettingsRow
          title="Saved recordings"
          onPress={() => router.push("/settings/recordings")}
        />
        <FieldGroup.SectionFooter>
          <Text>
            Audio downloads stay on this device. Cloud recordings can be
            downloaded from each note.
          </Text>
        </FieldGroup.SectionFooter>
        <SettingsError error={storage.error} />
      </FieldGroup.Section>
      {!auth.bypass && auth.billing.isPro && (
        <FieldGroup.Section title="Connected devices">
          {devices.isPending ? (
            <Text>Loading devices…</Text>
          ) : devices.error ? (
            <Button label="Try again" onPress={() => void devices.refetch()} />
          ) : (
            devices.data?.map((device) => (
              <SettingsRow
                key={device.deviceFingerprint}
                title={device.deviceName || "Unnamed device"}
                value={
                  device.deviceKind === "mobile"
                    ? "Phone"
                    : device.deviceKind === "watch"
                      ? "Watch"
                      : "Desktop"
                }
              />
            ))
          )}
        </FieldGroup.Section>
      )}
    </SettingsPage>
  );
}
