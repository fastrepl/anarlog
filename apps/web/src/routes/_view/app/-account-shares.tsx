import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useState } from "react";

import { DotsThree } from "@anlg/ui/components/icons";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import {
  deleteMyShare,
  deleteMyShares,
  listMyManagedShares,
  restrictMyShare,
} from "@/functions/account-shares";

import {
  accountCardClassName,
  accountMenuTriggerClassName,
  accountPillDangerClassName,
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
  const [confirmingAll, setConfirmingAll] = useState(false);

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
      queryClient.invalidateQueries({ queryKey: sharesQueryKey });
    },
  });

  const stopSharingAll = useMutation({
    mutationFn: async (shareIds: string[]) => {
      const result = await deleteMyShares({ data: { shareIds } });
      if (!result.success) {
        throw new Error(result.message);
      }
    },
    onSuccess: () => {
      setConfirmingAll(false);
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: sharesQueryKey });
    },
  });

  const shares = sharesQuery.data ?? [];
  const actionsDisabled =
    restrict.isPending || stopSharing.isPending || stopSharingAll.isPending;

  return (
    <>
      <div className="flex items-center justify-between gap-4">
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          Shared notes
        </h2>
        {!sharesQuery.isPending &&
          !sharesQuery.isError &&
          shares.length > 0 && (
            <button
              type="button"
              onClick={() => {
                if (confirmingAll) {
                  stopSharingAll.mutate(shares.map((share) => share.shareId));
                } else {
                  setConfirmingAll(true);
                }
              }}
              disabled={actionsDisabled}
              className={accountPillDangerClassName}
            >
              {stopSharingAll.isPending
                ? "Stopping..."
                : confirmingAll
                  ? "You sure?"
                  : "Stop sharing all"}
            </button>
          )}
      </div>
      <div className={cn([accountCardClassName, "mt-6"])}>
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
            You haven't shared any notes yet. Notes you share from the desktop
            app show up here.
          </p>
        ) : (
          <ul className="divide-y divide-[#ede7dc]">
            {shares.map((share) => (
              <li
                key={share.shareId}
                className="flex items-center justify-between gap-3 p-6 sm:px-8"
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
                <ShareRowMenu
                  shareId={share.shareId}
                  title={share.title || "Untitled note"}
                  canRestrict={share.scope !== "restricted"}
                  disabled={actionsDisabled}
                  restricting={
                    restrict.isPending && restrict.variables === share.shareId
                  }
                  stopping={
                    stopSharing.isPending &&
                    stopSharing.variables === share.shareId
                  }
                  onOpenChange={() => setConfirmingAll(false)}
                  onRestrict={() => restrict.mutate(share.shareId)}
                  onStopSharing={() => stopSharing.mutate(share.shareId)}
                />
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
        {stopSharingAll.isError && (
          <p className="px-6 pb-6 text-sm text-red-600 sm:px-8">
            {stopSharingAll.error?.message ||
              "Failed to stop sharing your notes"}
          </p>
        )}
      </div>
    </>
  );
}

function ShareRowMenu({
  shareId,
  title,
  canRestrict,
  disabled,
  restricting,
  stopping,
  onOpenChange,
  onRestrict,
  onStopSharing,
}: {
  shareId: string;
  title: string;
  canRestrict: boolean;
  disabled: boolean;
  restricting: boolean;
  stopping: boolean;
  onOpenChange: () => void;
  onRestrict: () => void;
  onStopSharing: () => void;
}) {
  return (
    <DropdownMenu
      onOpenChange={(open) => {
        if (open) {
          onOpenChange();
        }
      }}
    >
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          aria-label={`Actions for ${title}`}
          className={accountMenuTriggerClassName}
        >
          <DotsThree size={16} aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end" className="w-44">
        <AppFloatingPanel className={appFloatingMenuPanelClassName}>
          <DropdownMenuItem asChild className="cursor-pointer">
            <Link
              to="/share/$shareId/"
              params={{ shareId }}
              search={{ scheme: "anarlog" }}
            >
              Open
            </Link>
          </DropdownMenuItem>
          {canRestrict && (
            <DropdownMenuItem
              className="cursor-pointer"
              disabled={restricting}
              onSelect={onRestrict}
            >
              {restricting ? "Restricting..." : "Restrict"}
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem
            className="cursor-pointer text-red-700 focus:bg-red-50 focus:text-red-800"
            disabled={stopping}
            onSelect={onStopSharing}
          >
            {stopping ? "Stopping..." : "Stop sharing"}
          </DropdownMenuItem>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
