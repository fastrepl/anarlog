import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { getSupabaseBrowserClient } from "@/functions/supabase";

import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style2: {
    borderBottomColor: {
      ":is(*) > :not(:last-child)": "#ede7dc",
    },
    borderBottomStyle: {
      ":is(*) > :not(:last-child)": "solid",
    },
    borderBottomWidth: {
      ":is(*) > :not(:last-child)": "1px",
    },
  },
  style3: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 40rem)": "row",
    },
    gap: "1rem",
    padding: "1.5rem",
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
    paddingInline: {
      default: null,
      "@media (width >= 40rem)": "2rem",
    },
  },
  style4: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#181613",
  },
  style5: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style6: {
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBottom: "1.5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#dc2626",
  },
});
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
        .order("last_seen_at", {
          ascending: false,
        });
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
      queryClient.invalidateQueries({
        queryKey: devicesQueryKey,
      });
    },
  });
  const devices = devicesQuery.data ?? [];
  return (
    <div {...stylex.props(accountStyles.card)}>
      {devicesQuery.isPending ? (
        <p {...stylex.props(styles.style1)}>Checking your devices...</p>
      ) : devicesQuery.isError ? (
        <p {...stylex.props(styles.style1)}>
          Couldn't load your devices. Refresh to try again.
        </p>
      ) : devices.length === 0 ? (
        <p {...stylex.props(styles.style1)}>
          No synced devices yet. Devices appear here once sync is on.
        </p>
      ) : (
        <ul {...stylex.props(styles.style2)}>
          {devices.map((device) => (
            <li key={device.id} {...stylex.props(styles.style3)}>
              <div>
                <p {...stylex.props(styles.style4)}>
                  {device.device_name || "Unnamed device"}
                </p>
                <p {...stylex.props(styles.style5)}>
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
                {...stylex.props([
                  accountStyles.pill,
                  accountStyles.pillDanger,
                ])}
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
        <p {...stylex.props(styles.style6)}>
          {removeDevice.error?.message || "Failed to remove device"}
        </p>
      )}
    </div>
  );
}
