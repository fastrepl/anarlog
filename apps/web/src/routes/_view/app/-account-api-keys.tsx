import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { listKeys, revokeKey } from "@anlg/api-client";

import { getAuthorizedApiClient } from "./-account-api";
import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillDangerClassName,
} from "./-account-ui";

const apiKeysQueryKey = ["account-api-keys"];

export function ApiKeysSection() {
  const queryClient = useQueryClient();
  const session = useAccountSession();

  const keysQuery = useQuery({
    queryKey: apiKeysQueryKey,
    // Skip the SSR fetch: the browser-only access token throws on the
    // server, and this data is session-scoped anyway.
    enabled: typeof window !== "undefined",
    queryFn: async () => {
      const client = await getAuthorizedApiClient();
      const { data, error } = await listKeys({ client });
      if (error || !data) {
        throw new Error("Failed to load API keys");
      }
      return data;
    },
  });

  const revoke = useMutation({
    mutationFn: async (keyId: string) => {
      const client = await getAuthorizedApiClient();
      const { error } = await revokeKey({ client, path: { key_id: keyId } });
      if (error) {
        throw new Error("Failed to revoke API key");
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: apiKeysQueryKey });
    },
  });

  const keys = keysQuery.data ?? [];

  return (
    <div className={accountCardClassName}>
      {keysQuery.isPending || session.isPending ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Checking your API keys...
        </p>
      ) : keysQuery.isError ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          We could not load your API keys. Refresh the page to try again.
        </p>
      ) : keys.length === 0 ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          {/* Listing keys is not plan-gated, so an empty list is the only
              signal a free user gets; creating one is Pro-only. */}
          {session.data?.billing.isPro
            ? "No API keys yet. Create one from the desktop app to use the Cloud API."
            : "Cloud API keys come with Pro. Create and use them from the desktop app."}
        </p>
      ) : (
        <ul className="divide-y divide-[#ede7dc]">
          {keys.map((key) => (
            <li
              key={key.id}
              className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:px-8"
            >
              <div>
                <p className="text-base font-medium text-[#181613]">
                  {key.name}{" "}
                  <span className="font-mono text-sm text-[#756b5d]">
                    {key.key_prefix}...
                  </span>
                </p>
                <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                  Created{" "}
                  {new Date(key.created_at).toLocaleDateString("en-US", {
                    month: "long",
                    day: "numeric",
                  })}
                  {key.last_used_at
                    ? ` · last used ${new Date(
                        key.last_used_at,
                      ).toLocaleDateString("en-US", {
                        month: "long",
                        day: "numeric",
                      })}`
                    : " · never used"}
                </p>
              </div>
              <button
                onClick={() => revoke.mutate(key.id)}
                disabled={revoke.isPending}
                className={accountPillDangerClassName}
              >
                {revoke.isPending && revoke.variables === key.id
                  ? "Revoking..."
                  : "Revoke"}
              </button>
            </li>
          ))}
        </ul>
      )}
      {revoke.isError && (
        <p className="px-6 pb-6 text-sm text-red-600 sm:px-8">
          {revoke.error?.message || "Failed to revoke API key"}
        </p>
      )}
    </div>
  );
}
