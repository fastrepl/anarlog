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

export class AccountRequestError extends Error {
  constructor(
    message: string,
    readonly status?: number,
  ) {
    super(message);
  }
}

// Only a 403 means the plan does not cover the resource; every other failure is
// a load error and must not be reported to the user as a missing entitlement.
export function isPlanGated(error: unknown) {
  return error instanceof AccountRequestError && error.status === 403;
}
