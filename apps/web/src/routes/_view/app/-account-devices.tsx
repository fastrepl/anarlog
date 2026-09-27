import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { Desktop, DeviceMobile, Devices } from "@anlg/ui/components/icons";

import { getSyncDeviceAddon, updateSyncDeviceAddon } from "@/functions/billing";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import { inferSyncDeviceType } from "@/lib/sync-device-type";

import {
  accountCardClassName,
  accountPillDangerClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

const deviceRowSchema = z.object({
  id: z.string(),
  device_name: z.string().nullable(),
  created_at: z.string(),
  last_seen_at: z.string(),
});

const devicesQueryKey = ["account-sync-devices"];
const deviceAddonQueryKey = ["account-sync-device-addon"];

function formatAddonPrice(
  unitAmount: number | null,
  currency: string | null,
  period: "monthly" | "yearly" | null,
) {
  if (unitAmount == null || !currency || !period) {
    return null;
  }
  const amount = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: currency.toUpperCase(),
    minimumFractionDigits: unitAmount % 100 === 0 ? 0 : 2,
  }).format(unitAmount / 100);
  return `${amount}/${period === "yearly" ? "yr" : "mo"} per extra device`;
}

function DeviceAddonControls({ usedDevices }: { usedDevices: number }) {
  const queryClient = useQueryClient();
  const addonQuery = useQuery({
    queryKey: deviceAddonQueryKey,
    enabled: typeof window !== "undefined",
    queryFn: () => getSyncDeviceAddon(),
  });
  const updateAddon = useMutation({
    mutationFn: (quantity: number) =>
      updateSyncDeviceAddon({ data: { quantity } }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: deviceAddonQueryKey });
    },
  });

  const addon = addonQuery.data;
  if (!addon?.available) {
    return null;
  }

  const totalDevices = addon.includedDevices + addon.quantity;
  const priceText = formatAddonPrice(
    addon.unitAmount,
    addon.currency,
    addon.period,
  );
  const canRemove = addon.quantity > 0 && usedDevices <= totalDevices - 1;

  return (
    <div className="border-border-subtle flex flex-col gap-4 border-b p-6 sm:flex-row sm:items-center sm:justify-between sm:px-8">
      <div>
        <p className="text-color text-base font-medium">
          {usedDevices} of {totalDevices} device slots used
        </p>
        <p className="text-color-muted mt-1 text-sm leading-6">
          Pro includes {addon.includedDevices} devices
          {addon.quantity > 0
            ? ` plus ${addon.quantity} extra ${addon.quantity === 1 ? "slot" : "slots"}`
            : ""}
          .{priceText ? ` ${priceText}.` : ""}
        </p>
        {updateAddon.isError && (
          <p className="mt-1 text-sm text-red-600">
            {updateAddon.error?.message || "Couldn't update device slots"}
          </p>
        )}
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => updateAddon.mutate(addon.quantity - 1)}
          disabled={!canRemove || updateAddon.isPending}
          className={accountPillSecondaryClassName}
        >
          Remove slot
        </button>
        <button
          type="button"
          onClick={() => updateAddon.mutate(addon.quantity + 1)}
          disabled={addon.quantity >= addon.maxAddons || updateAddon.isPending}
          className={accountPillSecondaryClassName}
        >
          {updateAddon.isPending ? "Updating..." : "Add slot"}
        </button>
      </div>
    </div>
  );
}

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
      <DeviceAddonControls usedDevices={devices.length} />
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
