import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useState } from "react";

import {
  deleteMyShare,
  listMyManagedShares,
  restrictMyShare,
} from "@/functions/account-shares";

import {
  accountCardClassName,
  accountPillDangerClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

const SCOPE_LABELS = {
  public: "Public",
  link: "Anyone with the link",
  workspace: "Workspace",
  restricted: "Invited people only",
} as const;

const sharesQueryKey = ["account-managed-shares"];

export function SharedNotesSection() {
  const queryClient = useQueryClient();
  const [confirmingShareId, setConfirmingShareId] = useState<string | null>(
    null,
  );

  const sharesQuery = useQuery({
    queryKey: sharesQueryKey,
    // Skip the SSR fetch: this data is session-scoped and better fetched
    // client-side like the rest of the account queries.
    enabled: typeof window !== "undefined",
    queryFn: async () => {
      const result = await listMyManagedShares();
      if (result.status !== "ready") {
        throw new Error("Failed to load shared notes");
      }
      return result.shares;
    },
  });

  const restrict = useMutation({
    mutationFn: async (shareId: string) => {
      const result = await restrictMyShare({ data: { shareId } });
      if (!result.success) {
        throw new Error(result.message);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: sharesQueryKey });
    },
  });

  const stopSharing = useMutation({
    mutationFn: async (shareId: string) => {
      const result = await deleteMyShare({ data: { shareId } });
      if (!result.success) {
        throw new Error(result.message);
      }
    },
    onSuccess: () => {
      setConfirmingShareId(null);
      queryClient.invalidateQueries({ queryKey: sharesQueryKey });
    },
  });

  const shares = sharesQuery.data ?? [];

  return (
    <div className={accountCardClassName}>
      {sharesQuery.isPending ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Checking your shared notes...
        </p>
      ) : sharesQuery.isError ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Couldn't load your shared notes. Refresh to try again.
        </p>
      ) : shares.length === 0 ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          You haven't shared any notes yet. Notes you share from the desktop app
          show up here.
        </p>
      ) : (
        <ul className="divide-y divide-[#ede7dc]">
          {shares.map((share) => (
            <li
              key={share.shareId}
              className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:px-8"
            >
              <div className="min-w-0">
                <p className="truncate text-base font-medium text-[#181613]">
                  {share.title || "Untitled note"}
                </p>
                <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                  {SCOPE_LABELS[share.scope]} · updated{" "}
                  {new Date(share.updatedAt).toLocaleDateString("en-US", {
                    month: "long",
                    day: "numeric",
                  })}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Link
                  to="/share/$shareId/"
                  params={{ shareId: share.shareId }}
                  search={{ scheme: "anarlog" }}
                  className={accountPillSecondaryClassName}
                >
                  Open
                </Link>
                {share.scope !== "restricted" && (
                  <button
                    onClick={() => restrict.mutate(share.shareId)}
                    disabled={restrict.isPending}
                    className={accountPillSecondaryClassName}
                  >
                    {restrict.isPending && restrict.variables === share.shareId
                      ? "Restricting..."
                      : "Restrict"}
                  </button>
                )}
                <button
                  onClick={() => {
                    if (confirmingShareId === share.shareId) {
                      stopSharing.mutate(share.shareId);
                    } else {
                      setConfirmingShareId(share.shareId);
                    }
                  }}
                  disabled={stopSharing.isPending}
                  className={accountPillDangerClassName}
                >
                  {stopSharing.isPending &&
                  stopSharing.variables === share.shareId
                    ? "Stopping..."
                    : confirmingShareId === share.shareId
                      ? "You sure?"
                      : "Stop sharing"}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {restrict.isError && (
        <p className="px-6 pb-6 text-sm text-red-600 sm:px-8">
          {restrict.error?.message || "Failed to restrict shared note"}
        </p>
      )}
      {stopSharing.isError && (
        <p className="px-6 pb-6 text-sm text-red-600 sm:px-8">
          {stopSharing.error?.message || "Failed to stop sharing this note"}
        </p>
      )}
    </div>
  );
}
