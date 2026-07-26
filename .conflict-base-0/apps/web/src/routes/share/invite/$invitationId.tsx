import { useMutation, useQuery } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import {
  CircleCheckIcon,
  Clock3Icon,
  LogInIcon,
  MailCheckIcon,
  MailXIcon,
} from "lucide-react";

import { useShareRouteContinuation } from "@/components/share-route-continuation";
import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
  SharedNoteLoading,
  SharedNotePrompt,
  SharedNoteTransientError,
  SharedNoteUnavailable,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import { clearShareRouteContinuation } from "@/functions/share-route-continuation";
import {
  acceptSharedNoteInvitation,
  inspectMySharedNoteInvitation,
} from "@/functions/shared-notes";
import {
  clearShareRouteToken,
  prepareShareRoutePrivacy,
} from "@/lib/share-route-privacy";
import {
  getPrivateShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import { getInvitationRouteFailure } from "@/lib/shared-note-route-state";
import {
  type SharedNoteDesktopScheme,
  buildSharedNoteWebPath,
  sharedNoteDesktopSchemeSchema,
  invitationIdSchema,
} from "@/lib/shared-notes";

export const Route = createFileRoute("/share/invite/$invitationId")({
  validateSearch: (search) => ({
    scheme: sharedNoteDesktopSchemeSchema.parse(search.scheme),
  }),
  beforeLoad: () => prepareShareRoutePrivacy(),
  loader: async () => ({ user: await fetchUser() }),
  head: getPrivateShareHead,
  headers: () => privateShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const { user } = Route.useLoaderData();
  const { invitationId } = Route.useParams();
  const { scheme } = Route.useSearch();
  return (
    <ClientOnly fallback={<SharedNoteLoading />}>
      <InvitationClient
        invitationId={invitationId}
        signedIn={Boolean(user)}
        scheme={scheme}
      />
    </ClientOnly>
  );
}

function InvitationClient({
  invitationId,
  signedIn,
  scheme,
}: {
  invitationId: string;
  signedIn: boolean;
  scheme: SharedNoteDesktopScheme;
}) {
  const pathname = window.location.pathname;
  const continuation = useShareRouteContinuation(pathname);
  const validInvitationId = invitationIdSchema.safeParse(invitationId);
  const parsedInvitationId = validInvitationId.success
    ? validInvitationId.data
    : null;
  const invitationQuery = useQuery({
    queryKey: ["shared-note-invitation", parsedInvitationId],
    queryFn: async () => {
      if (!parsedInvitationId || !continuation.token) {
        throw new Error("shared note unavailable");
      }
      return inspectMySharedNoteInvitation({
        data: {
          invitationId: parsedInvitationId,
          token: continuation.token,
        },
      });
    },
    enabled: Boolean(signedIn && parsedInvitationId && continuation.token),
    gcTime: 0,
    retry: false,
    staleTime: Infinity,
  });
  const acceptMutation = useMutation({
    mutationFn: async () => {
      if (!continuation.token || !parsedInvitationId) {
        throw new Error("shared note unavailable");
      }

      const result = await acceptSharedNoteInvitation({
        data: {
          invitationId: parsedInvitationId,
          token: continuation.token,
        },
      });
      if (result.status !== "ready") {
        throw new Error("shared note unavailable");
      }
      return result.shareId;
    },
    onSuccess: async (shareId) => {
      await clearInvitationContinuation(pathname);
      window.location.assign(
        buildSharedNoteWebPath(
          `/share/${encodeURIComponent(shareId)}/`,
          scheme,
        ),
      );
    },
  });

  if (continuation.isPending) {
    return <SharedNoteLoading />;
  }

  if (continuation.isError) {
    return (
      <SharedNoteTransientError
        retry={() => {
          void continuation.retry();
        }}
      />
    );
  }

  if (!continuation.token || !parsedInvitationId) {
    return <SharedNoteUnavailable />;
  }

  if (!signedIn) {
    const search = new URLSearchParams({
      flow: "web",
      redirect: buildSharedNoteWebPath(pathname, scheme),
    });
    return (
      <SharedNotePrompt
        icon={<LogInIcon className="size-6" aria-hidden="true" />}
        title="Sign in to accept this invitation"
        description="Use the email address this note was shared with. Your invitation stays in this browser tab while you sign in."
        actions={
          <a
            href={`/auth/?${search.toString()}`}
            className={sharedPrimaryButtonClassName}
          >
            Sign in to Anarlog
          </a>
        }
      />
    );
  }

  if (invitationQuery.isPending) {
    return <SharedNoteLoading />;
  }

  const invitationFailure = getInvitationRouteFailure({
    acceptanceFailed: acceptMutation.isError,
    inspectionFailed: invitationQuery.isError,
    inspectionReady: invitationQuery.data?.status === "ready",
  });
  if (
    invitationFailure === "unavailable" ||
    invitationQuery.data?.status !== "ready"
  ) {
    return <SharedNoteUnavailable />;
  }

  const invitation = invitationQuery.data.invitation;

  if (invitation.status === "accepted") {
    const acceptedShareId = invitation.shareId;
    if (!acceptedShareId) {
      return <SharedNoteUnavailable />;
    }

    return (
      <SharedNotePrompt
        icon={<CircleCheckIcon className="size-6" aria-hidden="true" />}
        title="Invitation accepted"
        description="This note is already available in your shared notes."
        actions={
          <button
            type="button"
            className={sharedPrimaryButtonClassName}
            onClick={() => {
              void clearInvitationContinuation(pathname).then(() => {
                window.location.assign(
                  buildSharedNoteWebPath(
                    `/share/${encodeURIComponent(acceptedShareId)}/`,
                    scheme,
                  ),
                );
              });
            }}
          >
            Open note
          </button>
        }
      />
    );
  }

  if (invitation.status === "revoked") {
    return (
      <SharedNotePrompt
        icon={<MailXIcon className="size-6" aria-hidden="true" />}
        title="This invitation was revoked"
        description="The person who shared the note has withdrawn this invitation."
      />
    );
  }

  if (invitation.status === "expired") {
    return (
      <SharedNotePrompt
        icon={<Clock3Icon className="size-6" aria-hidden="true" />}
        title="This invitation has expired"
        description="Ask the person who shared the note to send a new invitation."
      />
    );
  }

  return (
    <SharedNotePrompt
      icon={<MailCheckIcon className="size-6" aria-hidden="true" />}
      title="A note was shared with you"
      description="Accept the invitation to add this note to your shared notes in Anarlog."
      actions={
        <>
          <button
            type="button"
            className={sharedPrimaryButtonClassName}
            disabled={acceptMutation.isPending}
            onClick={() => acceptMutation.mutate()}
          >
            {acceptMutation.isPending ? "Accepting…" : "Accept invitation"}
          </button>
          <button
            type="button"
            className={sharedSecondaryButtonClassName}
            onClick={() => {
              void clearInvitationContinuation(pathname).then(() => {
                window.location.assign("/");
              });
            }}
          >
            Not now
          </button>
          {invitationFailure === "accept-retry" && (
            <p className="basis-full text-sm text-red-700" role="status">
              We couldn’t accept this invitation. Please try again.
            </p>
          )}
        </>
      }
    />
  );
}

async function clearInvitationContinuation(pathname: string) {
  clearShareRouteToken(pathname);
  await clearShareRouteContinuation({ data: pathname }).catch(() => undefined);
}
