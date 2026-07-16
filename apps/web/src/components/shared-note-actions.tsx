import { useMutation } from "@tanstack/react-query";
import { ExternalLinkIcon } from "lucide-react";

import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
} from "@/components/shared-note-viewer";
import { getShareRouteToken } from "@/lib/share-route-privacy";
import {
  buildShareHandoffDeepLink,
  createLinkShareHandoff,
  createPublicShareHandoff,
} from "@/lib/shared-note-api";

export function AccountSharedNoteActions({ shareId }: { shareId: string }) {
  return (
    <SharedNoteActionButtons
      onOpen={() => {
        window.location.href = `hyprnote://share/open?mode=account&share_id=${encodeURIComponent(shareId)}`;
      }}
    />
  );
}

export function LinkSharedNoteActions({
  pathname,
  shareId,
}: {
  pathname: string;
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
      window.location.href = buildShareHandoffDeepLink(handoff.requestId);
    },
  });

  return (
    <SharedNoteActionButtons
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}

export function PublicSharedNoteActions({
  publicSlug,
}: {
  publicSlug: string;
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
      window.location.href = buildShareHandoffDeepLink(handoff.requestId);
    },
  });

  return (
    <SharedNoteActionButtons
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}

function SharedNoteActionButtons({
  error = false,
  isPending = false,
  onOpen,
}: {
  error?: boolean;
  isPending?: boolean;
  onOpen: () => void;
}) {
  return (
    <>
      <button
        type="button"
        className={sharedPrimaryButtonClassName}
        disabled={isPending}
        onClick={onOpen}
      >
        <ExternalLinkIcon className="mr-2 size-4" aria-hidden="true" />
        {isPending ? "Opening…" : "Open in Anarlog"}
      </button>
      <a
        href="/download/apple-silicon/"
        className={sharedSecondaryButtonClassName}
      >
        Try Anarlog
      </a>
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
