import { useQuery } from "@tanstack/react-query";

import { listConnections } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { useAuth } from "./context";

import { env } from "~/env";

export function useConnections(enabled = true) {
  const auth = useAuth();
  const userId = auth?.session?.user.id;

  return useQuery({
    queryKey: ["integration-status", userId],
    queryFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        return [];
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await listConnections({ client });
      if (error) {
        throw new Error("Failed to load integrations");
      }
      return data?.connections ?? [];
    },
    enabled: enabled && !!userId,
  });
}
