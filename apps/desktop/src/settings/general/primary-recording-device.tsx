import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { toast } from "@anlg/ui/components/ui/toast";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { getDeviceIdentity } from "~/auth/cloudsync-credentials";
import { requestSyncDevices } from "~/auth/sync-devices";
import { setSettingValue } from "~/settings/queries";
import { SETTING_CONTROL_CLASS, SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

export function PrimaryRecordingDeviceSelector() {
  const { t } = useLingui();
  const { session } = useAuth();
  const { isPro } = useBillingAccess();
  const value = useConfigValue("primary_recording_device");
  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- Cache by account; rotating access tokens must not become cache keys.
  const devicesQuery = useQuery({
    queryKey: ["sync-devices", session?.user.id],
    queryFn: ({ signal }) => requestSyncDevices(session!.access_token, signal),
    enabled: Boolean(session && isPro),
  });
  const identityQuery = useQuery({
    queryKey: ["local-recording-device"],
    queryFn: async () => {
      const identity = await getDeviceIdentity();
      if (!identity.fingerprint) throw new Error("Device identity unavailable");
      return identity;
    },
    enabled: Boolean(session && isPro),
  });
  const saveMutation = useMutation({
    mutationFn: (fingerprint: string) =>
      setSettingValue(
        "primary_recording_device",
        fingerprint === "ask" ? "" : fingerprint,
      ),
    onError: () => toast.error(t`Could not save primary device. Try again.`),
  });
  // The service does not identify device kinds. Only this app's identity is
  // known to be a desktop recorder; do not offer untyped phones or watches.
  const devices = (devicesQuery.data?.devices ?? []).filter(
    (device) => device.deviceFingerprint === identityQuery.data?.fingerprint,
  );
  const unavailable =
    value && !devices.some((device) => device.deviceFingerprint === value);

  return (
    <div>
      <SettingRow
        title={<Trans>Primary recording device</Trans>}
        description={
          !session ? (
            <Trans>Sign in to choose a primary recording device.</Trans>
          ) : !isPro ? (
            <Trans>
              Anarlog Pro coordinates recording across your devices.
            </Trans>
          ) : (
            <Trans>
              Prefer this device when multiple devices record the same meeting.
              Other devices can record when it is not recording.
            </Trans>
          )
        }
      >
        {(labelProps) => (
          <Select
            value={value || "ask"}
            onValueChange={(next) => saveMutation.mutate(next)}
          >
            <SelectTrigger
              {...labelProps}
              className={SETTING_CONTROL_CLASS}
              disabled={
                !session ||
                !isPro ||
                devicesQuery.isPending ||
                saveMutation.isPending
              }
            >
              <SelectValue placeholder={t`Choose a device`} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="ask">
                <Trans>Ask each meeting</Trans>
              </SelectItem>
              {devices.map((device) => (
                <SelectItem
                  key={device.deviceFingerprint}
                  value={device.deviceFingerprint}
                >
                  {device.deviceName || t`Unnamed device`}
                </SelectItem>
              ))}
              {unavailable ? (
                <SelectItem value={value} disabled>
                  <Trans>Unavailable device</Trans>
                </SelectItem>
              ) : null}
            </SelectContent>
          </Select>
        )}
      </SettingRow>
      {devicesQuery.isError || identityQuery.isError ? (
        <p role="alert" className="text-muted-foreground mt-2 text-xs">
          <Trans>Could not load your devices.</Trans>{" "}
          <button
            type="button"
            className="underline"
            onClick={() => {
              if (devicesQuery.isError) void devicesQuery.refetch();
              if (identityQuery.isError) void identityQuery.refetch();
            }}
          >
            <Trans>Try again</Trans>
          </button>
        </p>
      ) : null}
    </div>
  );
}
