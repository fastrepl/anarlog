import { useMutation } from "@tanstack/react-query";
import { ExternalLinkIcon } from "lucide-react";

import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
} from "@/components/shared-note-viewer";
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
  scheme,
  shareId,
}: {
  scheme: SharedNoteDesktopScheme;
  shareId: string;
}) {
  return (
    <SharedNoteActionButtons
      showAccountCreation={false}
      onOpen={() => {
        window.location.href = buildAccountShareDeepLink(shareId, scheme);
      }}
    />
  );
}

export function LinkSharedNoteActions({
  pathname,
  scheme,
  shareId,
}: {
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
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}

export function PublicSharedNoteActions({
  publicSlug,
  scheme,
}: {
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
  showAccountCreation = true,
}: {
  error?: boolean;
  isPending?: boolean;
  onOpen: () => void;
  showAccountCreation?: boolean;
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
      {showAccountCreation && (
        <a href="/auth/?flow=web" className={sharedSecondaryButtonClassName}>
          Create an account
        </a>
      )}
      <a
        href="/download/apple-silicon/"
        className={sharedSecondaryButtonClassName}
      >
        Download for Apple Silicon
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
