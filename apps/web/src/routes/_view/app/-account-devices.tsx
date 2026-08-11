import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { getSupabaseBrowserClient } from "@/functions/supabase";

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
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Checking your devices...
        </p>
      ) : devicesQuery.isError ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Couldn't load your devices. Refresh to try again.
        </p>
      ) : devices.length === 0 ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          No synced devices yet. Devices appear here once sync is on.
        </p>
      ) : (
        <ul className="divide-y divide-[#ede7dc]">
          {devices.map((device) => (
            <li
              key={device.id}
              className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:px-8"
            >
              <div>
                <p className="text-base font-medium text-[#181613]">
                  {device.device_name || "Unnamed device"}
                </p>
                <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                  Last seen{" "}
                  {new Date(device.last_seen_at).toLocaleDateString("en-US", {
                    month: "long",
                    day: "numeric",
                  })}
                </p>
              </div>
              <button
                onClick={() => removeDevice.mutate(device.id)}
                disabled={removeDevice.isPending}
                className={accountPillDangerClassName}
              >
                {removeDevice.isPending && removeDevice.variables === device.id
                  ? "Removing..."
                  : "Remove"}
              </button>
            </li>
          ))}
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
