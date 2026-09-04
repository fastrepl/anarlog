import { Trans, useLingui } from "@lingui/react/macro";
import {
  CircleNotch,
  ShareNetwork,
  Trash,
  UserPlus,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { Input } from "@anlg/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { cn } from "@anlg/utils";

import {
  deleteSharedResource,
  grantSharedResourceAccess,
  isTeamSharingUpsellError,
  listSharedResourceGuests,
  requireResourceSharingContext,
  revokeSharedResourceAccess,
  upsertSharedResource,
  type SharedResource,
  type SharedResourceType,
} from "./client";
import { sharedResourcesQueryKey, useSharedResources } from "./hooks";

import { useOptionalAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useAvailableShareWorkspaces } from "~/session-sharing/source";
import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";
import { useTabs } from "~/store/zustand/tabs";

export function ResourceShareButton(props: {
  resourceType: SharedResourceType;
  sourceId: string;
  title: string;
  buildPayload: () =>
    | Record<string, unknown>
    | Promise<Record<string, unknown>>;
}) {
  const auth = useOptionalAuth();
  if (!auth) return null;
  return <AuthenticatedResourceShareButton {...props} />;
}

function AuthenticatedResourceShareButton({
  resourceType,
  sourceId,
  title,
  buildPayload,
}: {
  resourceType: SharedResourceType;
  sourceId: string;
  title: string;
  buildPayload: () =>
    | Record<string, unknown>
    | Promise<Record<string, unknown>>;
}) {
  const { t } = useLingui();
  const auth = useOptionalAuth()!;
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const openNew = useTabs((state) => state.openNew);
  const [open, setOpen] = useState(false);
  const [email, setEmail] = useState("");
  const [workspaceDraft, setWorkspaceDraft] = useState<string | null>(null);
  const [teamUpsell, setTeamUpsell] = useState<"guest" | "workspace" | null>(
    null,
  );
  const resources = useSharedResources(resourceType);
  const owned = resources.data?.find(
    (resource) =>
      resource.accessKind === "owner" && resource.sourceId === sourceId,
  );
  const accountUserId = auth.session?.user.id;
  const workspaces = useAvailableShareWorkspaces(accountUserId);
  const selectedWorkspaceId =
    workspaceDraft ?? owned?.generalWorkspaceId ?? "personal";
  const guests = useQuery({
    queryKey: ["shared-resource-guests", owned?.shareId ?? ""],
    enabled: Boolean(open && owned?.shareId && auth.session && auth.supabase),
    retry: false,
    queryFn: () =>
      listSharedResourceGuests(
        requireResourceSharingContext(auth),
        owned!.shareId,
      ),
  });

  const publish = useMutation({
    mutationFn: async (generalWorkspaceId: string | null) =>
      upsertSharedResource(requireResourceSharingContext(auth), {
        resourceType,
        sourceId,
        title,
        payload: await buildPayload(),
        generalWorkspaceId,
      }),
    onSuccess: () => {
      setTeamUpsell(null);
      return invalidateSharing(queryClient, accountUserId, resourceType);
    },
    onError: (error) => {
      if (isTeamSharingUpsellError(error)) setTeamUpsell("workspace");
    },
  });
  const invite = useMutation({
    mutationFn: async (inviteeEmail: string) => {
      const share = await upsertSharedResource(
        requireResourceSharingContext(auth),
        {
          resourceType,
          sourceId,
          title,
          payload: await buildPayload(),
          generalWorkspaceId:
            selectedWorkspaceId === "personal" ? null : selectedWorkspaceId,
        },
      );
      await grantSharedResourceAccess(
        requireResourceSharingContext(auth),
        share.shareId,
        inviteeEmail,
      );
      return share.shareId;
    },
    onSuccess: async (shareId) => {
      setEmail("");
      setTeamUpsell(null);
      await Promise.all([
        invalidateSharing(queryClient, accountUserId, resourceType),
        queryClient.invalidateQueries({
          queryKey: ["shared-resource-guests", shareId],
        }),
      ]);
    },
    onError: (error) => {
      setTeamUpsell(isTeamSharingUpsellError(error) ? "guest" : null);
    },
  });
  const revoke = useMutation({
    mutationFn: (guestId: string) =>
      revokeSharedResourceAccess(requireResourceSharingContext(auth), guestId),
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["shared-resource-guests", owned?.shareId ?? ""],
      }),
  });
  const stopSharing = useMutation({
    mutationFn: () =>
      deleteSharedResource(
        requireResourceSharingContext(auth),
        requireOwnedShare(owned).shareId,
      ),
    onSuccess: async () => {
      await invalidateSharing(queryClient, accountUserId, resourceType);
      setOpen(false);
    },
  });
  const signedIn = Boolean(auth.session && !auth.session.user.is_anonymous);
  const error =
    publish.error ?? invite.error ?? revoke.error ?? stopSharing.error;

  return (
    <>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        className="text-muted-foreground hover:text-foreground gap-1.5 rounded-full"
        aria-label={t`Share ${title}`}
        onClick={() => setOpen(true)}
      >
        <ShareNetwork className="size-4" />
        <Trans>Share</Trans>
      </Button>
      <Dialog
        open={open}
        onOpenChange={(nextOpen) => {
          if (publish.isPending || invite.isPending) return;
          setOpen(nextOpen);
          if (!nextOpen) {
            setEmail("");
            setWorkspaceDraft(null);
            setTeamUpsell(null);
            publish.reset();
            invite.reset();
          }
        }}
      >
        <GlassDialogContent className="max-w-[420px]">
          <DialogHeader className="gap-1">
            <DialogTitle className="text-base">
              <Trans>Share {title}</Trans>
            </DialogTitle>
            <DialogDescription>
              <Trans>
                Share one item with a guest, or add it to a Team for everyone.
              </Trans>
            </DialogDescription>
          </DialogHeader>

          {!signedIn ? (
            <ShareGate
              description={t`Sign in to share folders, templates, and automations.`}
              action={
                <Button
                  type="button"
                  size="sm"
                  onClick={() => void auth.signIn()}
                >
                  <Trans>Sign in</Trans>
                </Button>
              }
            />
          ) : billing.isReady && !billing.isPaid && !owned ? (
            <ShareGate
              description={t`Folder access controls and reusable sharing are included with Pro.`}
              action={
                <Button type="button" size="sm" onClick={billing.upgradeToPro}>
                  <Trans>Upgrade to Pro</Trans>
                </Button>
              }
            />
          ) : (
            <div className="space-y-4">
              <div className="space-y-2">
                <p className="text-sm font-medium">
                  <Trans>Team library</Trans>
                </p>
                <div className="flex gap-2">
                  <Select
                    value={selectedWorkspaceId}
                    onValueChange={setWorkspaceDraft}
                  >
                    <SelectTrigger className="min-w-0 flex-1 rounded-xl">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="personal">
                        <Trans>Not shared with a Team</Trans>
                      </SelectItem>
                      {workspaces.map((workspace) => (
                        <SelectItem key={workspace.id} value={workspace.id}>
                          {workspace.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    type="button"
                    variant="outline"
                    className="rounded-xl"
                    disabled={publish.isPending || !billing.isPaid}
                    onClick={() =>
                      publish.mutate(
                        selectedWorkspaceId === "personal"
                          ? null
                          : selectedWorkspaceId,
                      )
                    }
                  >
                    {publish.isPending ? (
                      <CircleNotch className="size-4 animate-spin" />
                    ) : owned ? (
                      <Trans>Update</Trans>
                    ) : (
                      <Trans>Add</Trans>
                    )}
                  </Button>
                </div>
                {workspaces.length === 0 ? (
                  <button
                    type="button"
                    className="text-muted-foreground hover:text-foreground text-xs underline underline-offset-2"
                    onClick={() => {
                      setOpen(false);
                      openNew({ type: "settings", state: { tab: "team" } });
                    }}
                  >
                    <Trans>Create a Team to share a library</Trans>
                  </button>
                ) : null}
              </div>

              <div className="space-y-2">
                <p className="text-sm font-medium">
                  <Trans>Guest access</Trans>
                </p>
                <form
                  className="flex gap-2"
                  onSubmit={(event) => {
                    event.preventDefault();
                    const inviteeEmail = email.trim();
                    if (inviteeEmail) invite.mutate(inviteeEmail);
                  }}
                >
                  <Input
                    type="email"
                    value={email}
                    className="rounded-xl"
                    placeholder={t`guest@company.com`}
                    aria-label={t`Guest email`}
                    disabled={invite.isPending || !billing.isPaid}
                    onChange={(event) => {
                      setEmail(event.target.value);
                      setTeamUpsell(null);
                      invite.reset();
                    }}
                  />
                  <Button
                    type="submit"
                    className="rounded-xl"
                    disabled={
                      !email.trim() || invite.isPending || !billing.isPaid
                    }
                  >
                    {invite.isPending ? (
                      <CircleNotch className="size-4 animate-spin" />
                    ) : (
                      <UserPlus className="size-4" />
                    )}
                    <Trans>Invite</Trans>
                  </Button>
                </form>
                <p className="text-muted-foreground text-xs">
                  <Trans>
                    A guest can access one shared item for free. Add them to
                    your Team when they need more.
                  </Trans>
                </p>
              </div>

              {teamUpsell ? (
                <div className="border-border bg-accent/45 space-y-2 rounded-2xl border p-3">
                  <p className="text-sm font-medium">
                    {teamUpsell === "guest" ? (
                      <Trans>This guest needs Team access</Trans>
                    ) : (
                      <Trans>Unlock the Team library</Trans>
                    )}
                  </p>
                  <p className="text-muted-foreground text-xs leading-5">
                    {teamUpsell === "guest" ? (
                      <Trans>
                        They already have another shared item. Invite them to
                        your Team for folders, templates, and automations.
                      </Trans>
                    ) : (
                      <Trans>
                        Upgrade this workspace to share folders, templates, and
                        automations with everyone on the Team.
                      </Trans>
                    )}
                  </p>
                  <Button
                    type="button"
                    size="sm"
                    onClick={() => {
                      setOpen(false);
                      openNew({ type: "settings", state: { tab: "team" } });
                    }}
                  >
                    <Trans>Open Teams</Trans>
                  </Button>
                </div>
              ) : null}

              {owned ? (
                <div className="border-border/60 space-y-1 border-t pt-3">
                  {owned.generalWorkspaceId ? (
                    <AccessRow
                      label={
                        owned.workspaceName ? (
                          <Trans>Everyone in {owned.workspaceName}</Trans>
                        ) : (
                          <Trans>Everyone in the Team</Trans>
                        )
                      }
                    />
                  ) : null}
                  {guests.data?.map((guest) => (
                    <AccessRow
                      key={guest.guestId}
                      label={guest.email}
                      onRemove={() => revoke.mutate(guest.guestId)}
                    />
                  ))}
                </div>
              ) : null}

              {error && !teamUpsell ? (
                <p className="text-destructive text-xs">{error.message}</p>
              ) : null}

              <DialogFooter className="items-center sm:justify-between">
                {owned ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    className="text-destructive hover:text-destructive"
                    disabled={stopSharing.isPending}
                    onClick={() => stopSharing.mutate()}
                  >
                    <Trash className="size-4" />
                    <Trans>Stop sharing</Trans>
                  </Button>
                ) : (
                  <span />
                )}
                <GlassDialogCancelButton
                  type="button"
                  onClick={() => setOpen(false)}
                >
                  <Trans>Done</Trans>
                </GlassDialogCancelButton>
              </DialogFooter>
            </div>
          )}
        </GlassDialogContent>
      </Dialog>
    </>
  );
}

function ShareGate({
  description,
  action,
}: {
  description: string;
  action: React.ReactNode;
}) {
  return (
    <div className="bg-accent/45 flex items-center justify-between gap-3 rounded-2xl p-3">
      <p className="text-muted-foreground text-xs leading-5">{description}</p>
      <div className="shrink-0">{action}</div>
    </div>
  );
}

function AccessRow({
  label,
  onRemove,
}: {
  label: React.ReactNode;
  onRemove?: () => void;
}) {
  return (
    <div className="flex min-h-8 items-center justify-between gap-2 text-sm">
      <span className="min-w-0 truncate">{label}</span>
      {onRemove ? (
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className={cn(["h-7 rounded-full px-2 text-xs"])}
          onClick={onRemove}
        >
          <Trans>Remove</Trans>
        </Button>
      ) : (
        <span className="text-muted-foreground text-xs">
          <Trans>Team</Trans>
        </span>
      )}
    </div>
  );
}

function requireOwnedShare(
  resource: SharedResource | undefined,
): SharedResource {
  if (!resource) throw new Error("This item is not shared");
  return resource;
}

async function invalidateSharing(
  queryClient: ReturnType<typeof useQueryClient>,
  userId: string | undefined,
  resourceType: SharedResourceType,
) {
  await queryClient.invalidateQueries({
    queryKey: sharedResourcesQueryKey(userId, resourceType),
  });
}
