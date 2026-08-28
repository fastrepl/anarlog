import { Check, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { createKey, listKeys, revokeKey } from "@anlg/api-client";
import type { CreatedApiKey } from "@anlg/api-client";
import { fonts } from "@anlg/design-system/tokens.stylex";

import { authStyles } from "@/components/auth-shell";

import { getAuthorizedApiClient } from "./-account-api";
import { useAccountSession } from "./-account-session";
import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: "#ede7dc",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBlock: "1rem",
  },
  style2: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 48rem)": "row",
    },
    gap: ".75rem",
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "center",
    },
  },
  style3: {
    display: "flex",
    flexShrink: 0,
    gap: ".5rem",
  },
  style4: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1rem",
  },
  style5: {
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style6: {
    marginTop: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#dc2626",
  },
  style7: {
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: "#ede7dc",
    backgroundColor: "#fffaf0",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBlock: "1rem",
  },
  style8: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#181613",
  },
  style9: {
    marginTop: ".75rem",
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: ".75rem",
  },
  style10: {
    maxWidth: "100%",
    overflowX: "auto",
    borderRadius: ".5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#ede7dc",
    backgroundColor: "#fff",
    paddingInline: ".75rem",
    paddingBlock: ".5rem",
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#181613",
  },
  style11: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
  },
  style12: {
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style13: {
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
  style14: {
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
  style15: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#181613",
  },
  style16: {
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style17: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style18: {
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
      const { data, error } = await listKeys({
        client,
      });
      if (error || !data) {
        throw new Error("Failed to load API keys");
      }
      return data;
    },
  });
  const create = useMutation({
    mutationFn: async (name: string) => {
      const client = await getAuthorizedApiClient();
      const { data, error } = await createKey({
        client,
        body: {
          name,
        },
      });
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
      queryClient.invalidateQueries({
        queryKey: apiKeysQueryKey,
      });
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
      const { error } = await revokeKey({
        client,
        path: {
          key_id: keyId,
        },
      });
      if (error) {
        throw new Error("Failed to revoke API key");
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: apiKeysQueryKey,
      });
    },
  });
  const keys = keysQuery.data ?? [];
  const isPro = session.data?.billing.isPro === true;
  const showCreateControls =
    isPro && !keysQuery.isPending && !session.isPending && !keysQuery.isError;
  return (
    <div {...stylex.props(accountStyles.card)}>
      {showCreateControls && (
        <div {...stylex.props(styles.style1)}>
          {isCreating ? (
            <form
              onSubmit={handleCreateSubmit}
              {...stylex.props(styles.style2)}
            >
              <input
                type="text"
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                placeholder="Key name, e.g. my-script"
                maxLength={100}
                {...stylex.props(authStyles.input)}
                autoFocus
              />
              <div {...stylex.props(styles.style3)}>
                <button
                  type="submit"
                  disabled={create.isPending || !newKeyName.trim()}
                  {...stylex.props([
                    accountStyles.pill,
                    accountStyles.pillPrimary,
                  ])}
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
                  {...stylex.props([
                    accountStyles.pill,
                    accountStyles.pillSecondary,
                  ])}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <div {...stylex.props(styles.style4)}>
              <p {...stylex.props(styles.style5)}>
                Keys let your own tools call the Anarlog Cloud API.
              </p>
              <button
                onClick={() => {
                  setCreatedKey(null);
                  setCopiedKey(false);
                  setIsCreating(true);
                }}
                {...stylex.props([
                  accountStyles.pill,
                  accountStyles.pillSecondary,
                ])}
              >
                Create key
              </button>
            </div>
          )}
          {create.isError && (
            <p {...stylex.props(styles.style6)}>
              {create.error?.message || "Failed to create API key"}
            </p>
          )}
        </div>
      )}
      {createdKey && (
        <div {...stylex.props(styles.style7)}>
          <p {...stylex.props(styles.style8)}>
            {createdKey.name} is ready. Copy the key now; it won't be shown
            again.
          </p>
          <div {...stylex.props(styles.style9)}>
            <code {...stylex.props(styles.style10)}>{createdKey.key}</code>
            <button
              onClick={handleCopyKey}
              {...stylex.props([
                accountStyles.pill,
                accountStyles.pillSecondary,
              ])}
            >
              {copiedKey ? (
                <>
                  <Check {...stylex.props(styles.style11)} />
                  Copied
                </>
              ) : (
                <>
                  <Copy {...stylex.props(styles.style11)} />
                  Copy key
                </>
              )}
            </button>
          </div>
        </div>
      )}
      {keysQuery.isPending || session.isPending ? (
        <p {...stylex.props(styles.style12)}>Checking your API keys...</p>
      ) : keysQuery.isError ? (
        <p {...stylex.props(styles.style12)}>
          We could not load your API keys. Refresh the page to try again.
        </p>
      ) : keys.length === 0 ? (
        <p {...stylex.props(styles.style12)}>
          {/* Listing keys is not plan-gated, so an empty list is the only
              signal a free user gets; creating one is Pro-only. */}
          {isPro
            ? "No API keys yet. Create one to use the Cloud API."
            : "Cloud API keys come with Pro."}
        </p>
      ) : (
        <ul {...stylex.props(styles.style13)}>
          {keys.map((key) => (
            <li key={key.id} {...stylex.props(styles.style14)}>
              <div>
                <p {...stylex.props(styles.style15)}>
                  {key.name}{" "}
                  <span {...stylex.props(styles.style16)}>
                    {key.key_prefix}...
                  </span>
                </p>
                <p {...stylex.props(styles.style17)}>
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
                {...stylex.props([
                  accountStyles.pill,
                  accountStyles.pillDanger,
                ])}
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
        <p {...stylex.props(styles.style18)}>
          {revoke.error?.message || "Failed to revoke API key"}
        </p>
      )}
    </div>
  );
}
