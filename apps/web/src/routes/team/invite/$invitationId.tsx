import {
  CheckCircle,
  Clock,
  EnvelopeOpen,
  Prohibit,
  SignIn,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import { useShareRouteContinuation } from "@/components/share-route-continuation";
import {
  SharedNoteLoading,
  SharedNotePrompt,
  SharedNoteTransientError,
  SharedNoteUnavailable,
  sharedButtonStyles,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import { clearShareRouteContinuation } from "@/functions/share-route-continuation";
import {
  acceptWorkspaceInvitation,
  inspectMyWorkspaceInvitation,
} from "@/functions/team";
import {
  clearShareRouteToken,
  prepareShareRoutePrivacy,
} from "@/lib/share-route-privacy";
import { privateShareHeaders } from "@/lib/shared-note-meta";
import { getInvitationRouteFailure } from "@/lib/shared-note-route-state";
const styles = stylex.create({
  style1: {
    width: "1.5rem",
    height: "1.5rem",
  },
  style2: {
    flexBasis: "100%",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
});
const invitationIdSchema = z.string().uuid();
export const Route = createFileRoute("/team/invite/$invitationId")({
  beforeLoad: () => prepareShareRoutePrivacy(),
  loader: async () => ({
    user: await fetchUser(),
  }),
  head: () => ({
    meta: [
      {
        title: "Team invitation · Anarlog",
      },
      {
        name: "robots",
        content: "noindex, nofollow, noarchive, nosnippet",
      },
      {
        name: "referrer",
        content: "no-referrer",
      },
      {
        name: "ai-content",
        content: "private",
      },
    ],
  }),
  headers: () => privateShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});
function Component() {
  const { user } = Route.useLoaderData();
  const { invitationId } = Route.useParams();
  return (
    <ClientOnly fallback={<SharedNoteLoading />}>
      <InvitationClient invitationId={invitationId} signedIn={Boolean(user)} />
    </ClientOnly>
  );
}
function InvitationClient({
  invitationId,
  signedIn,
}: {
  invitationId: string;
  signedIn: boolean;
}) {
  const pathname = window.location.pathname;
  const continuation = useShareRouteContinuation(pathname);
  const validInvitationId = invitationIdSchema.safeParse(invitationId);
  const parsedInvitationId = validInvitationId.success
    ? validInvitationId.data
    : null;
  const invitationQuery = useQuery({
    queryKey: ["workspace-invitation", parsedInvitationId],
    queryFn: async () => {
      if (!parsedInvitationId || !continuation.token) {
        throw new Error("workspace invitation unavailable");
      }
      return inspectMyWorkspaceInvitation({
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
        throw new Error("workspace invitation unavailable");
      }
      const result = await acceptWorkspaceInvitation({
        data: {
          invitationId: parsedInvitationId,
          token: continuation.token,
        },
      });
      if (result.status !== "ready") {
        throw new Error("workspace invitation unavailable");
      }
      return result.workspaceId;
    },
    onSuccess: async () => {
      await clearInvitationContinuation(pathname);
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
      redirect: pathname,
    });
    return (
      <SharedNotePrompt
        icon={<SignIn {...stylex.props(styles.style1)} aria-hidden="true" />}
        title="Sign in to accept this invitation"
        description="Use the email address this workspace invitation was sent to. Your invitation stays in this browser tab while you sign in."
        actions={
          <a
            href={`/auth/?${search.toString()}`}
            {...stylex.props([
              sharedButtonStyles.base,
              sharedButtonStyles.primary,
            ])}
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
  if (invitation.status === "accepted" || acceptMutation.isSuccess) {
    return (
      <SharedNotePrompt
        icon={
          <CheckCircle {...stylex.props(styles.style1)} aria-hidden="true" />
        }
        title={`You've joined ${invitation.workspaceName}`}
        description="Open Anarlog on your computer to start collaborating in this workspace."
        actions={
          <a
            href="/download/"
            {...stylex.props([
              sharedButtonStyles.base,
              sharedButtonStyles.primary,
            ])}
          >
            Download Anarlog
          </a>
        }
      />
    );
  }
  if (invitation.status === "revoked") {
    return (
      <SharedNotePrompt
        icon={<Prohibit {...stylex.props(styles.style1)} aria-hidden="true" />}
        title="This invitation was revoked"
        description="Ask a workspace admin to send a new invitation."
      />
    );
  }
  if (invitation.status === "expired") {
    return (
      <SharedNotePrompt
        icon={<Clock {...stylex.props(styles.style1)} aria-hidden="true" />}
        title="This invitation has expired"
        description="Ask a workspace admin to send a new invitation."
      />
    );
  }
  return (
    <SharedNotePrompt
      icon={
        <EnvelopeOpen {...stylex.props(styles.style1)} aria-hidden="true" />
      }
      title={`Join ${invitation.workspaceName}`}
      description="Accept the invitation to become a member of this Anarlog workspace."
      actions={
        <>
          <button
            type="button"
            {...stylex.props([
              sharedButtonStyles.base,
              sharedButtonStyles.primary,
            ])}
            disabled={acceptMutation.isPending}
            onClick={() => acceptMutation.mutate()}
          >
            {acceptMutation.isPending ? "Accepting…" : "Accept invitation"}
          </button>
          <button
            type="button"
            {...stylex.props([
              sharedButtonStyles.base,
              sharedButtonStyles.secondary,
            ])}
            onClick={() => {
              void clearInvitationContinuation(pathname).then(() => {
                window.location.assign("/");
              });
            }}
          >
            Not now
          </button>
          {invitationFailure === "accept-retry" && (
            <p {...stylex.props(styles.style2)} role="status">
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
  await clearShareRouteContinuation({
    data: pathname,
  }).catch(() => undefined);
}
