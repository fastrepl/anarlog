import { DotsThree } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useState } from "react";

import {
  AppFloatingPanel,
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

import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1rem",
  },
  style2: {
    fontSize: "1.875rem",
    lineHeight: "1",
    "--tw-leading": "1",
    "--tw-font-weight": "600",
    fontWeight: "600",
    color: "#756b5d",
  },
  style3: {
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
    color: "#756b5d",
  },
  style4: {},
  style5: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: ".75rem",
    padding: "1.5rem",
    paddingInline: {
      default: null,
      "@media (width >= 40rem)": "2rem",
    },
  },
  style6: {
    minWidth: "0",
  },
  style7: {
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: "1rem",
    lineHeight: "1.5rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
    color: "#181613",
  },
  style8: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
    color: "#756b5d",
  },
  style9: {
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBottom: "1.5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#dc2626",
  },
  style10: {
    width: "11rem",
  },
  style11: {
    overflow: "hidden",
    padding: ".25rem",
  },
  style12: {
    cursor: "pointer",
  },
  style13: {
    cursor: "pointer",
    color: {
      default: "#b91c1c",
      ":focus": "#991b1b",
    },
    backgroundColor: {
      default: null,
      ":focus": "#fef2f2",
    },
  },
});
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
      const result = await restrictMyShare({
        data: {
          shareId,
        },
      });
      if (!result.success) {
        throw new Error(result.message);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: sharesQueryKey,
      });
    },
  });
  const stopSharing = useMutation({
    mutationFn: async (shareId: string) => {
      const result = await deleteMyShare({
        data: {
          shareId,
        },
      });
      if (!result.success) {
        throw new Error(result.message);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: sharesQueryKey,
      });
    },
  });
  const stopSharingAll = useMutation({
    mutationFn: async (shareIds: string[]) => {
      const result = await deleteMyShares({
        data: {
          shareIds,
        },
      });
      if (!result.success) {
        throw new Error(result.message);
      }
    },
    onSuccess: () => {
      setConfirmingAll(false);
    },
    onSettled: () => {
      queryClient.invalidateQueries({
        queryKey: sharesQueryKey,
      });
    },
  });
  const shares = sharesQuery.data ?? [];
  const actionsDisabled =
    restrict.isPending || stopSharing.isPending || stopSharingAll.isPending;
  return (
    <>
      <div {...stylex.props(styles.style1)}>
        <h2 {...stylex.props(styles.style2)}>Shared notes</h2>
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
              {...stylex.props([accountStyles.pill, accountStyles.pillDanger])}
            >
              {stopSharingAll.isPending
                ? "Stopping..."
                : confirmingAll
                  ? "You sure?"
                  : "Stop sharing all"}
            </button>
          )}
      </div>
      <div {...stylex.props([accountStyles.card, "mt-6"])}>
        {sharesQuery.isPending ? (
          <p {...stylex.props(styles.style3)}>Checking your shared notes...</p>
        ) : sharesQuery.isError ? (
          <p {...stylex.props(styles.style3)}>
            Couldn't load your shared notes. Refresh to try again.
          </p>
        ) : shares.length === 0 ? (
          <p {...stylex.props(styles.style3)}>
            You haven't shared any notes yet. Notes you share from the desktop
            app show up here.
          </p>
        ) : (
          <ul {...stylex.props(styles.style4)}>
            {shares.map((share) => (
              <li key={share.shareId} {...stylex.props(styles.style5)}>
                <div {...stylex.props(styles.style6)}>
                  <p {...stylex.props(styles.style7)}>
                    {share.title || "Untitled note"}
                  </p>
                  <p {...stylex.props(styles.style8)}>
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
          <p {...stylex.props(styles.style9)}>
            {restrict.error?.message || "Failed to restrict shared note"}
          </p>
        )}
        {stopSharing.isError && (
          <p {...stylex.props(styles.style9)}>
            {stopSharing.error?.message || "Failed to stop sharing this note"}
          </p>
        )}
        {stopSharingAll.isError && (
          <p {...stylex.props(styles.style9)}>
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
          {...stylex.props(accountStyles.menuTrigger)}
        >
          <DotsThree size={16} aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        variant="app"
        align="end"
        {...stylex.props(styles.style10)}
      >
        <AppFloatingPanel {...stylex.props(styles.style11)}>
          <DropdownMenuItem asChild {...stylex.props(styles.style12)}>
            <Link
              to="/share/$shareId/"
              params={{
                shareId,
              }}
              search={{
                scheme: "anarlog",
              }}
            >
              Open
            </Link>
          </DropdownMenuItem>
          {canRestrict && (
            <DropdownMenuItem
              {...stylex.props(styles.style12)}
              disabled={restricting}
              onSelect={onRestrict}
            >
              {restricting ? "Restricting..." : "Restrict"}
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem
            {...stylex.props(styles.style13)}
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
