import { Check, Copy } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { createKey, listKeys, revokeKey } from "@anlg/api-client";
import type { CreatedApiKey } from "@anlg/api-client";

import { authInputClassName } from "@/components/auth-shell";

import { getAuthorizedApiClient } from "./-account-api";
import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillDangerClassName,
  accountPillPrimaryClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

const apiKeysQueryKey = ["account-api-keys"];

export function ApiKeysSection() {
  const queryClient = useQueryClient();
  const session = useAccountSession();
  const [isCreating, setIsCreating] = useState(false);
  const [newKeyName, setNewKeyName] = useState("");
  const [createdKey, setCreatedKey] = useState<CreatedApiKey | null>(null);
  const [copiedKey, setCopiedKey] = useState(false);

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

  const create = useMutation({
    mutationFn: async (name: string) => {
      const client = await getAuthorizedApiClient();
      const { data, error } = await createKey({ client, body: { name } });
      if (error || !data) {
        throw new Error("Failed to create API key");
      }
      return data;
    },
    onSuccess: (data) => {
      setCreatedKey(data);
      setCopiedKey(false);
      setIsCreating(false);
      setNewKeyName("");
      queryClient.invalidateQueries({ queryKey: apiKeysQueryKey });
    },
  });

  const handleCreateSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const name = newKeyName.trim();
    if (name) {
      create.mutate(name);
    }
  };

  const handleCopyKey = async () => {
    if (!createdKey) {
      return;
    }
    await navigator.clipboard.writeText(createdKey.key);
    setCopiedKey(true);
    setTimeout(() => setCopiedKey(false), 2_000);
  };

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
  const isPro = session.data?.billing.isPro === true;
  const showCreateControls =
    isPro && !keysQuery.isPending && !session.isPending && !keysQuery.isError;

  return (
    <div className={accountCardClassName}>
      {showCreateControls && (
        <div className="border-b border-[#ede7dc] px-6 py-4 sm:px-8">
          {isCreating ? (
            <form
              onSubmit={handleCreateSubmit}
              className="flex flex-col gap-3 md:flex-row md:items-center"
            >
              <input
                type="text"
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                placeholder="Key name, e.g. my-script"
                maxLength={100}
                className={authInputClassName}
                autoFocus
              />
              <div className="flex shrink-0 gap-2">
                <button
                  type="submit"
                  disabled={create.isPending || !newKeyName.trim()}
                  className={accountPillPrimaryClassName}
                >
                  {create.isPending ? "Creating..." : "Create"}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setIsCreating(false);
                    setNewKeyName("");
                    create.reset();
                  }}
                  disabled={create.isPending}
                  className={accountPillSecondaryClassName}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <div className="flex items-center justify-between gap-4">
              <p className="text-sm leading-6 text-[#756b5d]">
                Keys let your own tools call the Anarlog Cloud API.
              </p>
              <button
                onClick={() => {
                  setCreatedKey(null);
                  setCopiedKey(false);
                  setIsCreating(true);
                }}
                className={accountPillSecondaryClassName}
              >
                Create key
              </button>
            </div>
          )}
          {create.isError && (
            <p className="mt-3 text-sm text-red-600">
              {create.error?.message || "Failed to create API key"}
            </p>
          )}
        </div>
      )}
      {createdKey && (
        <div className="border-b border-[#ede7dc] bg-[#fffaf0] px-6 py-4 sm:px-8">
          <p className="text-sm font-medium text-[#181613]">
            {createdKey.name} is ready. Copy the key now; it won't be shown
            again.
          </p>
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <code className="max-w-full overflow-x-auto rounded-lg border border-[#ede7dc] bg-white px-3 py-2 font-mono text-sm text-[#181613]">
              {createdKey.key}
            </code>
            <button
              onClick={handleCopyKey}
              className={accountPillSecondaryClassName}
            >
              {copiedKey ? (
                <>
                  <Check className="mr-2 size-4" />
                  Copied
                </>
              ) : (
                <>
                  <Copy className="mr-2 size-4" />
                  Copy key
                </>
              )}
            </button>
          </div>
        </div>
      )}
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
          {isPro
            ? "No API keys yet. Create one to use the Cloud API."
            : "Cloud API keys come with Pro."}
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
