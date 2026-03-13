import { useQuery } from "@tanstack/react-query";
import { jwtDecode } from "jwt-decode";

import { whoami } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";

export function useWhoAmI(enabled = true) {
  const authQuery = useQuery({
    queryKey: ["integration-status", "auth"],
    queryFn: async () => {
      const token = await getAccessToken();
      return {
        token,
        userId: jwtDecode<{ sub: string }>(token).sub,
      };
    },
    retry: false,
    enabled,
  });

  return useQuery({
    queryKey: ["whoami", authQuery.data?.userId],
    enabled: enabled && !!authQuery.data?.userId,
    queryFn: async () => {
      const client = createClient({
        baseUrl: env.VITE_API_URL,
        headers: { Authorization: `Bearer ${authQuery.data?.token}` },
      });
      const { data, error } = await whoami({ client });
      if (error) {
        throw new Error("Failed to load account info");
      }
      return data?.accounts ?? [];
    },
  });
}
