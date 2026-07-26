import { useMutation } from "@tanstack/react-query";

import { cn } from "@hypr/utils";

import { sharedPrimaryButtonClassName } from "@/components/shared-note-viewer";
import { getShareRouteToken } from "@/lib/share-route-privacy";
import {
  createLinkShareHandoff,
  createPublicShareHandoff,
} from "@/lib/shared-note-api";
import {
  buildAccountShareDeepLink,
  buildShareHandoffDeepLink,
  type SharedNoteDesktopScheme,
} from "@/lib/shared-notes";

export function AccountSharedNoteActions({
  canEdit,
  scheme,
  shareId,
}: {
  canEdit: boolean;
  scheme: SharedNoteDesktopScheme;
  shareId: string;
}) {
  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      onOpen={() => {
        window.location.href = buildAccountShareDeepLink(shareId, scheme);
      }}
    />
  );
}

export function LinkSharedNoteActions({
  canEdit,
  pathname,
  scheme,
  shareId,
}: {
  canEdit: boolean;
  pathname: string;
  scheme: SharedNoteDesktopScheme;
  shareId: string;
}) {
  const handoffMutation = useMutation({
    mutationFn: async () => {
      const token = getShareRouteToken(pathname);
      if (!token) {
        throw new Error("shared note unavailable");
      }
      const handoff = await createLinkShareHandoff(shareId, token);
      if (!handoff) {
        throw new Error("shared note unavailable");
      }
      return handoff;
    },
    onSuccess: (handoff) => {
      window.location.href = buildShareHandoffDeepLink(
        handoff.requestId,
        scheme,
      );
    },
  });

  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}

export function PublicSharedNoteActions({
  canEdit,
  publicSlug,
  scheme,
}: {
  canEdit: boolean;
  publicSlug: string;
  scheme: SharedNoteDesktopScheme;
}) {
  const handoffMutation = useMutation({
    mutationFn: async () => {
      const handoff = await createPublicShareHandoff(publicSlug);
      if (!handoff) {
        throw new Error("shared note unavailable");
      }
      return handoff;
    },
    onSuccess: (handoff) => {
      window.location.href = buildShareHandoffDeepLink(
        handoff.requestId,
        scheme,
      );
    },
  });

  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}

function SharedNoteActionButtons({
  canEdit,
  error = false,
  isPending = false,
  onOpen,
}: {
  canEdit: boolean;
  error?: boolean;
  isPending?: boolean;
  onOpen: () => void;
}) {
  return (
    <>
      <div className="group relative">
        <button
          type="button"
          className={sharedPrimaryButtonClassName}
          disabled={isPending}
          aria-describedby={canEdit ? "open-in-anarlog-tooltip" : undefined}
          onClick={onOpen}
        >
          <span className="hidden sm:inline">
            {isPending ? "Opening…" : "Open in Anarlog"}
          </span>
          <span className="sm:hidden">{isPending ? "Opening…" : "Open"}</span>
        </button>
        {canEdit && (
          <span
            id="open-in-anarlog-tooltip"
            role="tooltip"
            className={cn([
              "surface border-color-subtle text-color-muted pointer-events-none absolute top-full right-0 mt-2 w-max rounded-lg border px-2.5 py-1.5 text-xs shadow-lg",
              "translate-y-[-2px] opacity-0 transition-[opacity,transform] group-focus-within:translate-y-0 group-focus-within:opacity-100 group-hover:translate-y-0 group-hover:opacity-100",
            ])}
          >
            Open in Anarlog to edit
          </span>
        )}
      </div>
      {error && (
        <p
          className="text-color-muted basis-full text-right text-xs"
          role="status"
        >
          Anarlog couldn’t be opened. Try again.
        </p>
      )}
    </>
  );
}
