import { useMutation } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { LogInIcon, MailCheckIcon } from "lucide-react";

import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
  SharedNoteLoading,
  SharedNotePrompt,
  SharedNoteUnavailable,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import {
  clearShareRouteToken,
  getShareRouteToken,
  prepareShareRoutePrivacy,
} from "@/lib/share-route-privacy";
import {
  getPrivateShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import { invitationIdSchema, shareIdSchema } from "@/lib/shared-notes";

export const Route = createFileRoute("/share/invite/$invitationId")({
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
  const hasToken = Boolean(getShareRouteToken(pathname));
  const validInvitationId = invitationIdSchema.safeParse(invitationId);
  const acceptMutation = useMutation({
    mutationFn: async () => {
      const token = getShareRouteToken(pathname);
      if (!token || !validInvitationId.success) {
        throw new Error("shared note unavailable");
      }

      const supabase = getSupabaseBrowserClient();
      const { data, error } = await supabase.rpc(
        "accept_session_access_invitation",
        {
          p_invitation_id: validInvitationId.data,
          p_invite_token: token,
        },
      );
      if (error || !Array.isArray(data) || data.length !== 1) {
        throw new Error("shared note unavailable");
      }

      const shareId = shareIdSchema.safeParse(data[0]?.share_id);
      if (!shareId.success) {
        throw new Error("shared note unavailable");
      }
      return shareId.data;
    },
    onSuccess: (shareId) => {
      clearShareRouteToken(pathname);
      window.location.assign(`/share/${encodeURIComponent(shareId)}/`);
    },
  });

  if (!hasToken || !validInvitationId.success) {
    return <SharedNoteUnavailable />;
  }

  if (!signedIn) {
    const search = new URLSearchParams({
      flow: "web",
      redirect: pathname,
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
              clearShareRouteToken(pathname);
              window.location.assign("/");
            }}
          >
            Not now
          </button>
          {acceptMutation.isError && (
            <p className="text-color-muted basis-full text-sm" role="status">
              This invitation isn’t available for the signed-in account.
            </p>
          )}
        </>
      }
    />
  );
}
