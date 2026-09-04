import { useMutation, useQuery } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import {
  ArrowsClockwise,
  CheckCircle,
  Clock,
  EnvelopeOpen,
  Prohibit,
  SignIn,
  WarningCircle,
} from "@anlg/ui/components/icons";

import { useShareRouteContinuation } from "@/components/share-route-continuation";
import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
  SharedNoteLoading,
  SharedNotePrompt,
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

const invitationIdSchema = z.string().uuid();

export const Route = createFileRoute("/team/invite/$invitationId")({
  beforeLoad: () => prepareShareRoutePrivacy(),
  loader: async () => ({ user: await fetchUser() }),
  head: () => ({
    meta: [
      { title: "Team invitation · Anarlog" },
      {
        name: "robots",
        content: "noindex, nofollow, noarchive, nosnippet",
      },
      { name: "referrer", content: "no-referrer" },
      { name: "ai-content", content: "private" },
    ],
  }),
  headers: () => privateShareHeaders,
  pendingComponent: WorkspaceInvitationLoading,
  component: Component,
});

function Component() {
  const { user } = Route.useLoaderData();
  const { invitationId } = Route.useParams();
  return (
    <ClientOnly fallback={<WorkspaceInvitationLoading />}>
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
    return <WorkspaceInvitationLoading />;
  }

  if (continuation.isError) {
    return (
      <WorkspaceInvitationTransientError
        retry={() => {
          void continuation.retry();
        }}
      />
    );
  }

  if (!continuation.token || !parsedInvitationId) {
    return <WorkspaceInvitationUnavailable />;
  }

  if (!signedIn) {
    const search = new URLSearchParams({
      flow: "web",
      redirect: pathname,
    });
    return (
      <SharedNotePrompt
        headerLabel="Team invitation"
        icon={<SignIn className="size-6" aria-hidden="true" />}
        title="Sign in to accept this invitation"
        description="Use the email address this workspace invitation was sent to. Your invitation stays in this browser tab while you sign in."
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
    return <WorkspaceInvitationLoading />;
  }

  if (invitationQuery.isError || invitationQuery.data?.status === "error") {
    return (
      <WorkspaceInvitationTransientError
        retry={() => {
          void invitationQuery.refetch();
        }}
      />
    );
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
    return <WorkspaceInvitationUnavailable />;
  }

  const invitation = invitationQuery.data.invitation;

  if (invitation.status === "accepted" || acceptMutation.isSuccess) {
    return (
      <SharedNotePrompt
        headerLabel="Team invitation"
        icon={<CheckCircle className="size-6" aria-hidden="true" />}
        title={`You've joined ${invitation.workspaceName}`}
        description="Open Anarlog on your computer to start collaborating in this workspace."
        actions={
          <a href="/download/" className={sharedPrimaryButtonClassName}>
            Download Anarlog
          </a>
        }
      />
    );
  }

  if (invitation.status === "revoked") {
    return (
      <SharedNotePrompt
        headerLabel="Team invitation"
        icon={<Prohibit className="size-6" aria-hidden="true" />}
        title="This invitation was revoked"
        description="Ask a workspace admin to send a new invitation."
      />
    );
  }

  if (invitation.status === "expired") {
    return (
      <SharedNotePrompt
        headerLabel="Team invitation"
        icon={<Clock className="size-6" aria-hidden="true" />}
        title="This invitation has expired"
        description="Ask a workspace admin to send a new invitation."
      />
    );
  }

  return (
    <SharedNotePrompt
      headerLabel="Team invitation"
      icon={<EnvelopeOpen className="size-6" aria-hidden="true" />}
      title={`Join ${invitation.workspaceName}`}
      description="Accept the invitation to become a member of this Anarlog workspace."
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

function WorkspaceInvitationLoading() {
  return (
    <SharedNoteLoading
      headerLabel="Team invitation"
      loadingLabel="Loading team invitation"
    />
  );
}

function WorkspaceInvitationUnavailable() {
  return (
    <SharedNotePrompt
      headerLabel="Team invitation"
      icon={<WarningCircle className="size-6" aria-hidden="true" />}
      title="This team invitation isn’t available"
      description="Sign in with the email address that received this invitation, or ask the workspace admin for a new invitation."
    />
  );
}

function WorkspaceInvitationTransientError({ retry }: { retry: () => void }) {
  return (
    <SharedNotePrompt
      headerLabel="Team invitation"
      icon={<WarningCircle className="size-6" aria-hidden="true" />}
      title="We couldn’t load this team invitation"
      description="Anarlog had a temporary problem loading the invitation. Please try again."
      actions={
        <button
          type="button"
          className={sharedPrimaryButtonClassName}
          onClick={retry}
        >
          <ArrowsClockwise className="mr-2 size-4" aria-hidden="true" />
          Try again
        </button>
      }
    />
  );
}

async function clearInvitationContinuation(pathname: string) {
  clearShareRouteToken(pathname);
  await clearShareRouteContinuation({ data: pathname }).catch(() => undefined);
}
