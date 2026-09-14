import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { Desktop, DeviceMobile, Devices } from "@anlg/ui/components/icons";

import { getSupabaseBrowserClient } from "@/functions/supabase";
import { inferSyncDeviceType } from "@/lib/sync-device-type";

import {
  accountCardClassName,
  accountPillDangerClassName,
} from "./-account-ui";

const deviceRowSchema = z.object({
  id: z.string(),
  device_name: z.string().nullable(),
  created_at: z.string(),
  last_seen_at: z.string(),
});

const devicesQueryKey = ["account-sync-devices"];

export function DevicesSection() {
  const queryClient = useQueryClient();

  const devicesQuery = useQuery({
    queryKey: devicesQueryKey,
    // Skip the SSR fetch: the browser-only Supabase client throws on the
    // server, and this data is session-scoped anyway.
    enabled: typeof window !== "undefined",
    queryFn: async () => {
      const supabase = getSupabaseBrowserClient();
      const { data, error } = await supabase
        .from("sync_devices")
        .select("id, device_name, created_at, last_seen_at")
        .order("last_seen_at", { ascending: false });
      if (error) {
        throw new Error(error.message);
      }
      return z.array(deviceRowSchema).parse(data);
    },
  });

  const removeDevice = useMutation({
    mutationFn: async (deviceId: string) => {
      const supabase = getSupabaseBrowserClient();
      const { error } = await supabase
        .from("sync_devices")
        .delete()
        .eq("id", deviceId);
      if (error) {
        throw new Error(error.message);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: devicesQueryKey });
    },
  });

  const devices = devicesQuery.data ?? [];

  return (
    <div className={accountCardClassName}>
      {devicesQuery.isPending ? (
        <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
          Checking your devices...
        </p>
      ) : devicesQuery.isError ? (
        <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
          Couldn't load your devices. Refresh to try again.
        </p>
      ) : devices.length === 0 ? (
        <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
          No synced devices yet. Devices appear here once sync is on.
        </p>
      ) : (
        <ul className="divide-border-subtle divide-y">
          {devices.map((device) => {
            const deviceType = inferSyncDeviceType(device.device_name);
            const DeviceTypeIcon =
              deviceType === "mobile"
                ? DeviceMobile
                : deviceType === "desktop"
                  ? Desktop
                  : Devices;
            const deviceTypeLabel =
              deviceType === "mobile"
                ? "Mobile device"
                : deviceType === "desktop"
                  ? "Desktop device"
                  : "Device";

            return (
              <li
                key={device.id}
                className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:px-8"
              >
                <div className="flex items-center gap-3">
                  <span
                    role="img"
                    aria-label={deviceTypeLabel}
                    title={deviceTypeLabel}
                    className="surface-subtle border-color-subtle text-color-muted flex size-10 shrink-0 items-center justify-center rounded-xl border"
                  >
                    <DeviceTypeIcon size={20} aria-hidden="true" />
                  </span>
                  <div>
                    <p className="text-color text-base font-medium">
                      {device.device_name || "Unnamed device"}
                    </p>
                    <p className="text-color-muted mt-1 text-sm leading-6">
                      Last seen{" "}
                      {new Date(device.last_seen_at).toLocaleDateString(
                        "en-US",
                        {
                          month: "long",
                          day: "numeric",
                        },
                      )}
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => removeDevice.mutate(device.id)}
                  disabled={removeDevice.isPending}
                  className={accountPillDangerClassName}
                >
                  {removeDevice.isPending &&
                  removeDevice.variables === device.id
                    ? "Removing..."
                    : "Remove"}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {removeDevice.isError && (
        <p className="px-6 pb-6 text-sm text-red-600 sm:px-8">
          {removeDevice.error?.message || "Failed to remove device"}
        </p>
      )}
    </div>
  );
}
