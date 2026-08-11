import { createClient } from "@anlg/api-client/client";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";

export async function getAuthorizedApiClient() {
  const token = await getAccessToken();
  return createClient({
    baseUrl: env.VITE_API_URL,
    headers: { Authorization: `Bearer ${token}` },
  });
}
