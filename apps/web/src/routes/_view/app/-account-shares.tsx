import {
  useInfiniteQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useDeferredValue, useState } from "react";

import { DotsThree, MagnifyingGlass } from "@anlg/ui/components/icons";
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
  const [searchQuery, setSearchQuery] = useState("");
  const deferredSearchQuery = useDeferredValue(searchQuery.trim());

  const sharesQuery = useInfiniteQuery({
    queryKey: [...sharesQueryKey, deferredSearchQuery],
    initialPageParam: null as {
      publishedAt: string;
      shareId: string;
    } | null,
    // Skip the SSR fetch: this data is session-scoped and better fetched
    // client-side like the rest of the account queries.
    enabled: typeof window !== "undefined",
    queryFn: async ({ pageParam }) => {
      const result = await listMyManagedShares({
        data: {
          query: deferredSearchQuery || undefined,
          afterPublishedAt: pageParam?.publishedAt,
          afterShareId: pageParam?.shareId,
        },
      });
      if (result.status !== "ready") {
        throw new Error("Failed to load shared notes");
      }
      return result;
    },
    getNextPageParam: (lastPage) => lastPage.nextCursor,
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
    mutationFn: async () => {
      const result = await deleteMyShares();
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

  const shares = sharesQuery.data?.pages.flatMap((page) => page.shares) ?? [];
  const actionsDisabled =
    restrict.isPending || stopSharing.isPending || stopSharingAll.isPending;

  return (
    <>
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="text-brand-dark font-hand text-3xl leading-none font-semibold">
          Shared notes
        </h2>
        {!sharesQuery.isPending &&
          !sharesQuery.isError &&
          shares.length > 0 && (
            <button
              type="button"
              onClick={() => {
                if (confirmingAll) {
                  stopSharingAll.mutate();
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
      {!sharesQuery.isError && (
        <div
          role="search"
          className="surface border-color-subtle text-color-muted focus-within:border-color-bright mt-6 flex h-11 items-center gap-3 rounded-full border px-4"
        >
          <MagnifyingGlass size={18} aria-hidden="true" />
          <input
            type="search"
            aria-label="Search shared note titles"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder="Search note titles"
            className="text-color placeholder:text-color-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </div>
      )}
      <div className="mt-6">
        {sharesQuery.isPending ? (
          <div className={accountCardClassName}>
            <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
              Checking your shared notes...
            </p>
          </div>
        ) : sharesQuery.isError ? (
          <div className={accountCardClassName}>
            <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
              Couldn't load your shared notes. Refresh to try again.
            </p>
          </div>
        ) : shares.length === 0 && !deferredSearchQuery ? (
          <div className={accountCardClassName}>
            <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
              You haven't shared any notes yet. Notes you share from the desktop
              app show up here.
            </p>
          </div>
        ) : shares.length === 0 ? (
          <div className={accountCardClassName}>
            <p className="text-color-muted p-6 text-sm leading-6 sm:p-8">
              No shared notes match “{searchQuery.trim()}”.
            </p>
          </div>
        ) : (
          <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            {shares.map((share) => {
              const cardContent = (
                <>
                  <div className="surface-subtle border-color-subtle text-color-muted h-36 overflow-hidden rounded-xl border p-4 pr-11 text-xs leading-5">
                    <p className="line-clamp-5">
                      {share.hasSnapshot
                        ? share.preview || "No text preview available yet."
                        : "Preview isn't available yet. You can still manage sharing from the menu."}
                    </p>
                  </div>
                  <p className="text-color mt-4 truncate text-base font-medium">
                    {share.title || "Untitled note"}
                  </p>
                  <p className="text-color-muted mt-1 text-xs leading-5">
                    {SCOPE_LABELS[share.scope]} · updated{" "}
                    {new Date(share.updatedAt).toLocaleDateString("en-US", {
                      month: "long",
                      day: "numeric",
                    })}
                  </p>
                </>
              );

              return (
                <li
                  key={share.shareId}
                  className={cn([
                    "surface border-color-subtle relative min-w-0 overflow-hidden rounded-[20px] border transition",
                    share.hasSnapshot &&
                      "group hover:border-color-bright hover:shadow-lg",
                  ])}
                >
                  {share.hasSnapshot ? (
                    <Link
                      to="/share/$shareId/"
                      params={{ shareId: share.shareId }}
                      search={{ scheme: "anarlog" }}
                      className="block h-full p-4 pb-5"
                    >
                      {cardContent}
                    </Link>
                  ) : (
                    <div className="h-full p-4 pb-5">{cardContent}</div>
                  )}
                  <ShareCardMenu
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
              );
            })}
          </ul>
        )}
        {sharesQuery.hasNextPage && (
          <button
            type="button"
            onClick={() => sharesQuery.fetchNextPage()}
            disabled={sharesQuery.isFetchingNextPage}
            className="surface border-color-subtle text-color hover:border-color-bright mx-auto mt-6 flex h-10 items-center justify-center rounded-full border px-5 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50"
          >
            {sharesQuery.isFetchingNextPage ? "Loading..." : "Load more"}
          </button>
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

function ShareCardMenu({
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
          className={cn([
            accountMenuTriggerClassName,
            "surface absolute top-6 right-6 shadow-sm",
          ])}
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
