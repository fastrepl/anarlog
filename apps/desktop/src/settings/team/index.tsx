import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useDebounceValue } from "usehooks-ts";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import { Avatar } from "@anlg/ui/components/avatar";
import {
  CircleNotch,
  DotsThree,
  PaperPlaneTilt,
  Plus,
  Trash,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { Input } from "@anlg/ui/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@anlg/ui/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn } from "@anlg/utils";

import {
  acceptMyWorkspaceInvitation,
  checkWorkspaceShareSlugAvailability,
  claimWorkspaceDomain,
  createWorkspace,
  declineMyWorkspaceInvitation,
  deleteWorkspace,
  getWorkspaceAccess,
  getWorkspacePolicy,
  getWorkspaceUsageOverview,
  leaveWorkspace,
  listWorkspaceInvitations,
  listWorkspaceMembers,
  removeMember,
  renameWorkspace,
  requireTeamContext,
  revokeInvitation,
  rotateWorkspaceScimToken,
  setMemberRole,
  setWorkspaceLogo,
  setWorkspacePolicy,
  setWorkspaceShareSlug,
  transferOwnership,
  listOwnershipRequests,
  respondOwnershipRequest,
  type MyWorkspaceInvitation,
  type WorkspaceCapability,
  type WorkspaceMember,
  type WorkspacePolicy,
  type WorkspaceRole,
} from "./client";
import { WorkspaceEmailAutoJoin } from "./email-auto-join";
import {
  deliverWorkspaceInvitation,
  getTeamSenderName,
  reportWorkspaceInvitation,
} from "./invitation";
import { WorkspaceLogoButton, WorkspaceLogoMark } from "./logo-button";
import { MY_WORKSPACES_QUERY_KEY, useMyWorkspacesWithMirror } from "./mirror";
import {
  MY_INVITATIONS_QUERY_KEY,
  useMyWorkspaceInvitations,
} from "./my-invitations";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import {
  cancelScheduledCapture,
  listScheduledCaptures,
} from "~/enterprise-capture/client";
import { env } from "~/env";
import { SettingsPageTitle } from "~/settings/page-title";
import { PlanGate } from "~/settings/plan-gate";
import { SettingSwitchRow } from "~/settings/setting-row";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";
import { buildWebAppUrl } from "~/shared/utils";

export function SettingsTeam() {
  const auth = useAuth();
  const { isPro } = useBillingAccess();
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const signedIn = Boolean(auth.supabase && auth.session);

  // Shares the query (and therefore the mirror refresh) with the app-level
  // mount, so opening this page is never what makes sharing scopes appear.
  const workspaces = useMyWorkspacesWithMirror();
  const invitations = useMyWorkspaceInvitations();
  const selectedWorkspace =
    isCreating || !workspaces.data
      ? undefined
      : (workspaces.data.find(
          (workspace) => workspace.workspaceId === selectedId,
        ) ?? workspaces.data[0]);

  const create = useMutation({
    mutationFn: (name: string) =>
      createWorkspace(requireTeamContext(auth), name),
    onSuccess: (result) => {
      setIsCreating(false);
      setSelectedId(result.workspaceId);
      void queryClient.invalidateQueries({
        queryKey: [MY_WORKSPACES_QUERY_KEY],
      });
    },
  });

  if (!signedIn) {
    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title={<Trans>Teams</Trans>} />
        <p className="text-muted-foreground text-sm">
          <Trans>Sign in to create a shared workspace for your team.</Trans>
        </p>
      </div>
    );
  }

  const createForm = (
    <PlanGate allowed={isPro} plan="pro">
      <CreateWorkspaceForm
        onCreate={(name) => create.mutate(name)}
        pending={create.isPending}
        error={create.error?.message}
        placeholder={t`Acme`}
      />
    </PlanGate>
  );

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Teams</Trans>} />

      {invitations.data && invitations.data.length > 0 ? (
        <PendingInvitations
          invitations={invitations.data}
          onJoined={(workspaceId) => {
            setIsCreating(false);
            setSelectedId(workspaceId);
          }}
        />
      ) : null}

      {workspaces.isPending ? (
        <TeamSkeleton />
      ) : workspaces.data && workspaces.data.length > 0 ? (
        <div className="flex flex-col gap-8">
          <WorkspaceTabs
            workspaces={workspaces.data}
            selectedId={selectedWorkspace?.workspaceId ?? null}
            isCreating={isCreating}
            canCreate
            onSelect={(workspaceId) => {
              setIsCreating(false);
              setSelectedId(workspaceId);
            }}
            onCreate={() => setIsCreating(true)}
          />
          {isCreating ? (
            createForm
          ) : selectedWorkspace ? (
            <WorkspacePanel
              key={selectedWorkspace.workspaceId}
              workspaceId={selectedWorkspace.workspaceId}
              workspaceName={selectedWorkspace.name}
              workspaceShareSlug={selectedWorkspace.shareSlug ?? null}
              workspaceLogoDataUrl={selectedWorkspace.logoDataUrl ?? null}
              workspaceRole={selectedWorkspace.role ?? "member"}
              onWorkspaceRenamed={() => {
                void queryClient.invalidateQueries({
                  queryKey: [MY_WORKSPACES_QUERY_KEY],
                });
              }}
              onWorkspaceLeft={() => {
                setSelectedId(null);
                void queryClient.invalidateQueries({
                  queryKey: [MY_WORKSPACES_QUERY_KEY],
                });
              }}
            />
          ) : null}
        </div>
      ) : (
        createForm
      )}
    </div>
  );
}

function PendingInvitations({
  invitations,
  onJoined,
}: {
  invitations: MyWorkspaceInvitation[];
  onJoined: (workspaceId: string) => void;
}) {
  const auth = useAuth();
  const { t } = useLingui();
  const queryClient = useQueryClient();

  const accept = useMutation({
    mutationFn: (invitation: MyWorkspaceInvitation) =>
      acceptMyWorkspaceInvitation(
        requireTeamContext(auth),
        invitation.invitationId,
      ),
    onSuccess: (result, invitation) => {
      onJoined(result.workspaceId);
      void queryClient.invalidateQueries({
        queryKey: [MY_WORKSPACES_QUERY_KEY],
      });
      void queryClient.invalidateQueries({
        queryKey: [MY_INVITATIONS_QUERY_KEY],
      });
      sonnerToast.success(t`Joined ${invitation.workspaceName}`);
      sonnerToast.dismiss(`team-invitation:${invitation.invitationId}`);
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });
  const decline = useMutation({
    mutationFn: (invitation: MyWorkspaceInvitation) =>
      declineMyWorkspaceInvitation(
        requireTeamContext(auth),
        invitation.invitationId,
      ),
    onSuccess: (_result, invitation) => {
      void queryClient.invalidateQueries({
        queryKey: [MY_INVITATIONS_QUERY_KEY],
      });
      sonnerToast.dismiss(`team-invitation:${invitation.invitationId}`);
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  return (
    <section className="flex flex-col gap-3">
      <h2 className="font-sans text-lg font-semibold">
        <Trans>Invitations</Trans>
      </h2>
      {invitations.map((invitation) => {
        const accepting =
          accept.isPending &&
          accept.variables?.invitationId === invitation.invitationId;
        const declining =
          decline.isPending &&
          decline.variables?.invitationId === invitation.invitationId;
        return (
          <div
            key={invitation.invitationId}
            className="flex items-center gap-3"
          >
            <WorkspaceLogoMark
              logoDataUrl={invitation.workspaceLogoDataUrl}
              className="size-8 rounded-lg"
            />
            <div className="flex min-w-0 flex-col">
              <span className="text-sm font-medium">
                {invitation.workspaceName}
              </span>
              <span className="text-muted-foreground text-xs">
                {invitation.invitedByEmail ? (
                  <Trans>Invited by {invitation.invitedByEmail}</Trans>
                ) : (
                  <Trans>Invited to join</Trans>
                )}
              </span>
            </div>
            <div className="ml-auto flex items-center gap-2">
              <Button
                type="button"
                size="sm"
                disabled={accept.isPending || decline.isPending}
                onClick={() => accept.mutate(invitation)}
              >
                {accepting ? (
                  <CircleNotch className="size-4 animate-spin" />
                ) : null}
                <Trans>Accept</Trans>
              </Button>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                disabled={accept.isPending || decline.isPending}
                onClick={() => decline.mutate(invitation)}
              >
                {declining ? (
                  <CircleNotch className="size-4 animate-spin" />
                ) : null}
                <Trans>Decline</Trans>
              </Button>
            </div>
          </div>
        );
      })}
    </section>
  );
}

function WorkspaceTabs({
  workspaces,
  selectedId,
  isCreating,
  canCreate,
  onSelect,
  onCreate,
}: {
  workspaces: {
    workspaceId: string;
    name: string;
    logoDataUrl?: string | null;
  }[];
  selectedId: string | null;
  isCreating: boolean;
  canCreate: boolean;
  onSelect: (workspaceId: string) => void;
  onCreate: () => void;
}) {
  const { t } = useLingui();

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {workspaces.map((workspace) => {
        const selected = workspace.workspaceId === selectedId && !isCreating;
        return (
          <button
            key={workspace.workspaceId}
            type="button"
            aria-label={workspace.name}
            aria-pressed={selected}
            onClick={() => onSelect(workspace.workspaceId)}
            className={cn([
              "flex items-center gap-2 rounded-full py-1.5 pr-3 pl-1.5 text-sm transition-colors",
              selected
                ? "bg-muted text-foreground font-medium"
                : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
            ])}
          >
            <WorkspaceLogoMark
              logoDataUrl={workspace.logoDataUrl ?? null}
              className="size-5 rounded-md"
            />
            <span>{workspace.name}</span>
          </button>
        );
      })}
      {canCreate ? (
        <button
          type="button"
          aria-label={t`Create a shared workspace`}
          aria-pressed={isCreating}
          onClick={onCreate}
          className={cn([
            "flex size-8 items-center justify-center rounded-full transition-colors",
            isCreating
              ? "bg-muted text-foreground"
              : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
          ])}
        >
          <Plus className="size-4" />
        </button>
      ) : null}
    </div>
  );
}

function CreateWorkspaceForm({
  onCreate,
  pending,
  error,
  placeholder,
}: {
  onCreate: (name: string) => void;
  pending: boolean;
  error?: string;
  placeholder: string;
}) {
  const [name, setName] = useState("");
  const trimmed = name.trim();

  return (
    <section className="flex max-w-xl flex-col gap-4">
      <h2 className="font-sans text-lg font-semibold">
        <Trans>Create a shared workspace</Trans>
      </h2>
      <p className="text-muted-foreground text-xs leading-5">
        <Trans>
          Invite teammates, share notes across the workspace, and manage who has
          access. Your personal notes stay private.
        </Trans>
      </p>
      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          if (trimmed) onCreate(trimmed);
        }}
      >
        <Input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={placeholder}
          maxLength={120}
          className="bg-card h-9 max-w-xs shadow-none"
        />
        <Button type="submit" size="sm" disabled={!trimmed || pending}>
          {pending ? (
            <CircleNotch className="size-4 animate-spin" />
          ) : (
            <Plus className="size-4" />
          )}
          <Trans>Create</Trans>
        </Button>
      </form>
      {error && <p className="text-destructive text-xs">{error}</p>}
    </section>
  );
}

function WorkspacePanel({
  workspaceId,
  workspaceName,
  workspaceShareSlug,
  workspaceLogoDataUrl,
  workspaceRole,
  onWorkspaceRenamed,
  onWorkspaceLeft,
}: {
  workspaceId: string;
  workspaceName: string;
  workspaceShareSlug: string | null;
  workspaceLogoDataUrl: string | null;
  workspaceRole: WorkspaceRole;
  // Renaming keeps the panel where it is; leaving or deleting must drop the
  // selection because the workspace is gone.
  onWorkspaceRenamed: () => void;
  onWorkspaceLeft: () => void;
}) {
  const auth = useAuth();
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [transferTarget, setTransferTarget] = useState<WorkspaceMember | null>(
    null,
  );
  const [isAcceptTransferOpen, setIsAcceptTransferOpen] = useState(false);
  const [email, setEmail] = useState("");
  const [nameDraft, setNameDraft] = useState(workspaceName);
  const [isOpeningBilling, setIsOpeningBilling] = useState(false);
  const [isInviteDialogOpen, setIsInviteDialogOpen] = useState(false);
  const [isDeleteWorkspaceDialogOpen, setIsDeleteWorkspaceDialogOpen] =
    useState(false);
  const isManager = workspaceRole === "owner" || workspaceRole === "admin";

  const access = useQuery({
    queryKey: ["team-access", workspaceId],
    queryFn: () => getWorkspaceAccess(requireTeamContext(auth), workspaceId),
    retry: false,
  });
  const hasCapability = (capability: WorkspaceCapability) =>
    access.data?.capabilities.includes(capability) === true;
  const isEnterpriseWorkspace = access.data?.tier === "enterprise";
  const canManageWorkspace =
    isManager && hasCapability("team.manage_workspace");
  const canManageMembers = isManager && hasCapability("team.manage_members");
  const canManagePolicies =
    isManager && isEnterpriseWorkspace && hasCapability("team.manage_policies");
  const canViewUsage =
    isManager && isEnterpriseWorkspace && hasCapability("team.view_usage");
  const canUseCustomSubdomain =
    isManager &&
    isEnterpriseWorkspace &&
    hasCapability("team.custom_subdomain");
  const canConfigureSso = isManager && hasCapability("enterprise.sso");
  const canConfigureScim = isManager && hasCapability("enterprise.scim");
  const canConfigureRetention =
    isManager && hasCapability("enterprise.retention");
  const canUseEnterpriseCapture =
    isManager && hasCapability("enterprise.capture");
  const hasPaidWorkspacePlan =
    access.data?.tier === "team" || access.data?.tier === "enterprise";

  // The roster, invitation, and seat RPCs are manager-only, so a plain member
  // gets a permission error rather than data. Retrying cannot fix that.
  const members = useQuery({
    queryKey: ["team-members", workspaceId],
    queryFn: () => listWorkspaceMembers(requireTeamContext(auth), workspaceId),
    retry: false,
  });
  // Credentials and the Supabase client are not serializable cache identity.
  // eslint-disable-next-line @tanstack/query/exhaustive-deps
  const ownershipRequests = useQuery({
    queryKey: ["team-ownership-requests", workspaceId, auth.session?.user.id],
    queryFn: () => listOwnershipRequests(requireTeamContext(auth), workspaceId),
    retry: false,
    refetchInterval: 15_000,
  });
  const ownershipRequest = ownershipRequests.data?.[0];
  const invitations = useQuery({
    queryKey: ["team-invitations", workspaceId],
    queryFn: () =>
      listWorkspaceInvitations(requireTeamContext(auth), workspaceId),
    retry: false,
  });
  const usage = useQuery({
    queryKey: ["team-usage", workspaceId],
    queryFn: () =>
      getWorkspaceUsageOverview(requireTeamContext(auth), workspaceId),
    retry: false,
    enabled: canViewUsage,
  });
  const policy = useQuery({
    queryKey: ["team-policy", workspaceId],
    queryFn: () => getWorkspacePolicy(requireTeamContext(auth), workspaceId),
    retry: false,
    enabled: canManagePolicies,
  });

  const refresh = () => {
    void queryClient.invalidateQueries({
      queryKey: ["team-ownership-requests", workspaceId],
    });
    void queryClient.invalidateQueries({
      queryKey: ["team-access", workspaceId],
    });
    void queryClient.invalidateQueries({
      queryKey: ["team-members", workspaceId],
    });
    void queryClient.invalidateQueries({
      queryKey: ["team-invitations", workspaceId],
    });
    void queryClient.invalidateQueries({
      queryKey: ["team-usage", workspaceId],
    });
    void queryClient.invalidateQueries({
      queryKey: ["team-policy", workspaceId],
    });
  };

  const invite = useMutation({
    mutationFn: (value: string) =>
      deliverWorkspaceInvitation({
        context: requireTeamContext(auth),
        workspaceId,
        workspaceName,
        email: value,
        senderName: getTeamSenderName(auth.session?.user ?? {}),
      }),
    onSuccess: (result) => {
      setEmail("");
      setIsInviteDialogOpen(false);
      refresh();
      reportWorkspaceInvitation(result.deliveredBy);
    },
  });
  const changeRole = useMutation({
    mutationFn: (input: { userId: string; role: "admin" | "member" }) =>
      setMemberRole(
        requireTeamContext(auth),
        workspaceId,
        input.userId,
        input.role,
      ),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: (userId: string) =>
      removeMember(requireTeamContext(auth), workspaceId, userId),
    onSuccess: refresh,
  });
  const cancelInvite = useMutation({
    mutationFn: (invitationId: string) =>
      revokeInvitation(requireTeamContext(auth), invitationId),
    onSuccess: refresh,
  });
  const resendInvite = useMutation({
    mutationFn: (invitation: { email: string }) =>
      deliverWorkspaceInvitation({
        context: requireTeamContext(auth),
        workspaceId,
        workspaceName,
        email: invitation.email,
        senderName: getTeamSenderName(auth.session?.user ?? {}),
      }),
    onSuccess: (result) => {
      refresh();
      reportWorkspaceInvitation(result.deliveredBy);
    },
  });
  const transfer = useMutation({
    mutationFn: (userId: string) =>
      transferOwnership(requireTeamContext(auth), workspaceId, userId),
    onSuccess: () => {
      setTransferTarget(null);
      refresh();
    },
  });
  const respondTransfer = useMutation({
    mutationFn: (action: "accept" | "decline" | "cancel") => {
      if (!ownershipRequest)
        throw new Error("Ownership request is no longer available");
      return respondOwnershipRequest(
        requireTeamContext(auth),
        workspaceId,
        ownershipRequest.id,
        action,
      );
    },
    onSuccess: () => {
      setIsAcceptTransferOpen(false);
      refresh();
      onWorkspaceRenamed();
    },
  });
  const rename = useMutation({
    mutationFn: (value: string) =>
      renameWorkspace(requireTeamContext(auth), workspaceId, value),
    onSuccess: onWorkspaceRenamed,
  });
  const setLogo = useMutation({
    mutationFn: (logoDataUrl: string | null) =>
      setWorkspaceLogo(requireTeamContext(auth), workspaceId, logoDataUrl),
    onSuccess: onWorkspaceRenamed,
  });
  const leave = useMutation({
    mutationFn: () => leaveWorkspace(requireTeamContext(auth), workspaceId),
    onSuccess: onWorkspaceLeft,
  });
  const destroy = useMutation({
    mutationFn: () => deleteWorkspace(requireTeamContext(auth), workspaceId),
    onSuccess: () => {
      setIsDeleteWorkspaceDialogOpen(false);
      onWorkspaceLeft();
    },
    onError: () => setIsDeleteWorkspaceDialogOpen(false),
  });

  const viewerId = auth.session?.user.id;
  const viewerRole = workspaceRole;
  const trimmedEmail = email.trim();
  const hasAdminControls =
    canManagePolicies ||
    canUseCustomSubdomain ||
    canViewUsage ||
    (canUseEnterpriseCapture && Boolean(env.VITE_ENTERPRISE_API_URL));
  const actionError =
    changeRole.error?.message ??
    remove.error?.message ??
    cancelInvite.error?.message ??
    resendInvite.error?.message ??
    respondTransfer.error?.message ??
    ownershipRequests.error?.message ??
    transfer.error?.message ??
    rename.error?.message ??
    setLogo.error?.message ??
    leave.error?.message ??
    destroy.error?.message;

  const submitRename = (value: string) => {
    const next = value.trim();
    if (next && next !== workspaceName) rename.mutate(next);
    else setNameDraft(workspaceName);
  };

  const openTeamBilling = async () => {
    if (isOpeningBilling) return;
    setIsOpeningBilling(true);
    try {
      const url = await buildWebAppUrl("/app/team-checkout", {
        workspace_id: workspaceId,
        period: "monthly",
        quantity: String(Math.max(access.data?.usedSeats ?? 1, 1)),
      });
      await openUrlWithInstruction(url, "billing", (value) =>
        openerCommands.openUrl(value, null),
      );
    } finally {
      setIsOpeningBilling(false);
    }
  };

  return (
    <div className="flex flex-col gap-8">
      <div className="flex items-center gap-4">
        <WorkspaceLogoButton
          logoDataUrl={workspaceLogoDataUrl}
          label={t`Change workspace logo`}
          removeLabel={t`Remove workspace logo`}
          canManage={canManageWorkspace}
          pending={setLogo.isPending}
          onUpload={(dataUrl) => setLogo.mutate(dataUrl)}
          onRemove={() => setLogo.mutate(null)}
        />
        {canManageWorkspace ? (
          <Input
            value={nameDraft}
            onChange={(event) => setNameDraft(event.target.value)}
            maxLength={120}
            aria-label={t`Workspace name`}
            className="bg-card h-9 max-w-sm shadow-none"
            onBlur={(event) => submitRename(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              } else if (event.key === "Escape") {
                setNameDraft(workspaceName);
                event.currentTarget.blur();
              }
            }}
          />
        ) : (
          <p className="truncate text-sm font-medium">{workspaceName}</p>
        )}
        {rename.isPending ? (
          <CircleNotch className="text-muted-foreground size-4 animate-spin" />
        ) : null}
      </div>

      {actionError && <p className="text-destructive text-xs">{actionError}</p>}

      {isManager ? (
        <section className="flex max-w-xl flex-col gap-3 rounded-lg border p-4">
          <div>
            <h2 className="text-sm font-medium">
              {access.isPending ? (
                <span
                  className="bg-muted block h-5 w-20 animate-pulse rounded"
                  aria-hidden="true"
                />
              ) : hasPaidWorkspacePlan ? (
                <Trans>Team plan</Trans>
              ) : (
                <Trans>Start Team</Trans>
              )}
            </h2>
            <p className="text-muted-foreground mt-1 text-xs leading-5">
              <Trans>
                Pro for every member, shared workspaces, roles, and centralized
                billing. $20 per person monthly or $200 yearly.
              </Trans>
            </p>
          </div>
          <Button
            type="button"
            size="sm"
            className="w-fit"
            disabled={access.isPending || isOpeningBilling}
            onClick={() => void openTeamBilling()}
          >
            {access.isPending || isOpeningBilling ? (
              <CircleNotch className="size-4 animate-spin" />
            ) : null}
            {access.isPending ? (
              <span className="sr-only">
                <Trans>Team plan</Trans>
              </span>
            ) : hasPaidWorkspacePlan ? (
              <Trans>Manage Team billing</Trans>
            ) : (
              <Trans>Continue to Team checkout</Trans>
            )}
          </Button>
        </section>
      ) : null}

      <section className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="font-sans text-lg font-semibold">
            <Trans>Members</Trans>
          </h2>
          {canManageMembers ? (
            <Button
              type="button"
              size="sm"
              onClick={() => {
                invite.reset();
                setIsInviteDialogOpen(true);
              }}
            >
              <Plus className="size-4" />
              <Trans>Add members</Trans>
            </Button>
          ) : null}
        </div>

        {workspaceRole === "owner" && canManageMembers ? (
          <WorkspaceEmailAutoJoin workspaceId={workspaceId} />
        ) : null}

        {ownershipRequest ? (
          <div
            role="status"
            className="flex flex-wrap items-center gap-3 rounded-lg border p-3 text-sm"
          >
            <p>
              <Trans>
                Ownership transfer Pending. Current ownership and permissions
                remain unchanged until the proposed owner accepts.
              </Trans>
            </p>
            {ownershipRequest.targetUserId === viewerId ? (
              <>
                <Button
                  size="sm"
                  disabled={respondTransfer.isPending}
                  onClick={() => {
                    respondTransfer.reset();
                    setIsAcceptTransferOpen(true);
                  }}
                >
                  <Trans>Review transfer</Trans>
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={respondTransfer.isPending}
                  onClick={() => respondTransfer.mutate("decline")}
                >
                  <Trans>Decline</Trans>
                </Button>
              </>
            ) : ownershipRequest.ownerUserId === viewerId ? (
              <Button
                size="sm"
                variant="outline"
                disabled={respondTransfer.isPending}
                onClick={() => respondTransfer.mutate("cancel")}
              >
                <Trans>Cancel transfer</Trans>
              </Button>
            ) : null}
          </div>
        ) : null}
        {members.isPending ? (
          <TeamSkeleton />
        ) : members.isError ? (
          <p className="text-muted-foreground text-sm">
            <Trans>
              Only workspace admins can see who has access. You are a member of
              this workspace.
            </Trans>
          </p>
        ) : (
          <div className="border-border overflow-x-auto rounded-lg border">
            <table
              className="border-border [&_td]:border-border [&_th]:border-border w-full border-collapse text-left text-sm [&_td:not(:last-child)]:border-r [&_th:not(:last-child)]:border-r"
              aria-label={t`Members`}
            >
              <thead className="bg-muted/40 text-muted-foreground border-border border-b text-xs">
                <tr>
                  <th scope="col" className="px-4 py-3 font-medium">
                    <Trans>Name</Trans>
                  </th>
                  <th scope="col" className="px-4 py-3 font-medium">
                    <Trans>Email</Trans>
                  </th>
                  <th scope="col" className="px-4 py-3 font-medium">
                    <Trans>Permissions</Trans>
                  </th>
                  <th scope="col" className="w-12 px-4 py-3">
                    <span className="sr-only">
                      <Trans>Actions</Trans>
                    </span>
                  </th>
                </tr>
              </thead>
              <tbody className="divide-border divide-y">
                {members.data?.map((member) => (
                  <MemberRow
                    key={member.userId}
                    member={member}
                    isViewer={member.userId === viewerId}
                    viewerRole={isManager ? viewerRole : undefined}
                    canManageMembers={canManageMembers}
                    onRoleChange={(role) =>
                      changeRole.mutate({ userId: member.userId, role })
                    }
                    onRemove={() => remove.mutate(member.userId)}
                    ownershipPending={
                      ownershipRequest?.targetUserId === member.userId
                    }
                    transferDisabled={
                      ownershipRequests.isPending ||
                      ownershipRequests.isError ||
                      Boolean(ownershipRequest) ||
                      transfer.isPending
                    }
                    onTransfer={() => {
                      transfer.reset();
                      setTransferTarget(member);
                    }}
                  />
                ))}
                {invitations.data?.map((invitation) => (
                  <tr key={invitation.invitationId}>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-3">
                        <Avatar
                          seed={invitation.email}
                          label={invitation.email}
                          size={32}
                          className="rounded-full"
                        />
                        <span className="text-muted-foreground whitespace-nowrap">
                          <Trans>Invitation pending</Trans>
                        </span>
                      </div>
                    </td>
                    <td className="text-muted-foreground px-4 py-3">
                      {invitation.email}
                    </td>
                    <td className="text-muted-foreground px-4 py-3">—</td>
                    <td className="px-4 py-3">
                      {isManager ? (
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button
                              size="icon"
                              variant="ghost"
                              className="size-8"
                              aria-label={t`Actions for ${invitation.email}`}
                              disabled={
                                resendInvite.isPending || cancelInvite.isPending
                              }
                            >
                              <DotsThree className="size-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            {canManageMembers ? (
                              <DropdownMenuItem
                                onSelect={() =>
                                  resendInvite.mutate({
                                    email: invitation.email,
                                  })
                                }
                              >
                                <PaperPlaneTilt className="size-4" />
                                <Trans>Resend invitation</Trans>
                              </DropdownMenuItem>
                            ) : null}
                            <DropdownMenuItem
                              className="text-destructive"
                              onSelect={() =>
                                cancelInvite.mutate(invitation.invitationId)
                              }
                            >
                              <Trash className="size-4" />
                              <Trans>Cancel invitation</Trans>
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      ) : null}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {hasAdminControls ? (
        <section className="flex flex-col gap-8">
          <h2 className="font-sans text-lg font-semibold">
            <Trans>Admin</Trans>
          </h2>
          {policy.data ? (
            <WorkspacePolicyForm
              workspaceId={workspaceId}
              policy={policy.data}
              canConfigureSso={canConfigureSso}
              canConfigureScim={canConfigureScim}
              canConfigureRetention={canConfigureRetention}
              onSaved={refresh}
            />
          ) : null}
          {canUseCustomSubdomain ? (
            <WorkspaceShareDomainForm
              workspaceId={workspaceId}
              workspaceShareSlug={workspaceShareSlug}
              onSaved={() => {
                refresh();
                onWorkspaceRenamed();
              }}
            />
          ) : null}
          {canViewUsage && usage.data ? (
            <div className="flex flex-col gap-3">
              <h3 className="text-sm font-medium">
                <Trans>Usage</Trans>
              </h3>
              <p className="text-muted-foreground text-xs leading-5">
                <Trans>
                  Workspace activity from metadata only. Note content stays
                  unreadable on the server.
                </Trans>
              </p>
              <dl className="grid max-w-lg grid-cols-2 gap-x-6 gap-y-3 text-sm">
                {[
                  [t`Members`, usage.data.memberCount],
                  [
                    t`Seats`,
                    usage.data.seatLimit != null
                      ? `${usage.data.usedSeats} / ${usage.data.seatLimit}`
                      : usage.data.usedSeats,
                  ],
                  [t`Devices`, usage.data.enrolledDevices],
                  [t`Shares (30d)`, usage.data.sharesCreated30d],
                ].map(([label, value]) => (
                  <div key={label}>
                    <dt className="text-muted-foreground text-xs">{label}</dt>
                    <dd className="mt-1 font-medium tabular-nums">{value}</dd>
                  </div>
                ))}
              </dl>
            </div>
          ) : null}
          {canUseEnterpriseCapture ? (
            <UpcomingCaptureBots workspaceId={workspaceId} />
          ) : null}
        </section>
      ) : null}

      <div className="flex min-w-0 items-center justify-between gap-4">
        <p className="text-muted-foreground min-w-0 text-xs">
          {viewerRole === "owner" ? (
            <Trans>
              Deleting removes the workspace for everyone. Transfer ownership
              first if you only want to leave.
            </Trans>
          ) : (
            <Trans>Leaving gives up your access to shared notes here.</Trans>
          )}
        </p>
        {viewerRole === "owner" ? (
          <Button
            size="sm"
            variant="destructive"
            className="shrink-0"
            disabled={destroy.isPending}
            onClick={() => setIsDeleteWorkspaceDialogOpen(true)}
          >
            <Trans>Delete workspace</Trans>
          </Button>
        ) : (
          <Button
            size="sm"
            variant="destructive"
            className="shrink-0"
            disabled={leave.isPending}
            onClick={() => {
              if (confirm(t`Leave ${workspaceName}?`)) leave.mutate();
            }}
          >
            <Trans>Leave workspace</Trans>
          </Button>
        )}
      </div>
      <DestructiveConfirmationDialog
        open={transferTarget !== null}
        onOpenChange={(open) => {
          if (!open) setTransferTarget(null);
        }}
        title={t`Request ownership transfer?`}
        description={
          <>
            <Trans>
              {transferTarget?.email} must accept before becoming owner. Until
              then, ownership and permissions stay the same. After acceptance,
              you become an admin and they control the workspace, including
              deletion.
            </Trans>
            {transfer.error ? (
              <span role="alert">{transfer.error.message}</span>
            ) : null}
          </>
        }
        confirmLabel={<Trans>Request transfer</Trans>}
        isPending={transfer.isPending}
        onConfirm={() => {
          if (transferTarget) transfer.mutate(transferTarget.userId);
        }}
      />
      <DestructiveConfirmationDialog
        open={isAcceptTransferOpen && Boolean(ownershipRequest)}
        onOpenChange={setIsAcceptTransferOpen}
        title={t`Accept workspace ownership?`}
        description={
          <>
            <Trans>
              You will become the owner of {workspaceName}, with control over
              its members, billing, and deletion. The current owner will become
              an admin.
            </Trans>
            {respondTransfer.error ? (
              <span role="alert">{respondTransfer.error.message}</span>
            ) : null}
          </>
        }
        confirmLabel={<Trans>Accept ownership</Trans>}
        isPending={respondTransfer.isPending}
        onConfirm={() => respondTransfer.mutate("accept")}
      />
      <DestructiveConfirmationDialog
        open={isDeleteWorkspaceDialogOpen}
        onOpenChange={setIsDeleteWorkspaceDialogOpen}
        title={t`Delete ${workspaceName} for everyone?`}
        description={
          <Trans>
            Deleting removes the workspace for everyone. Transfer ownership
            first if you only want to leave.
          </Trans>
        }
        confirmLabel={<Trans>Delete workspace</Trans>}
        isPending={destroy.isPending}
        onConfirm={() => destroy.mutate()}
      />
      <Dialog
        open={isInviteDialogOpen}
        onOpenChange={(open) => {
          if (!open && invite.isPending) return;
          setIsInviteDialogOpen(open);
          if (!open) {
            setEmail("");
            invite.reset();
          }
        }}
      >
        <GlassDialogContent>
          <DialogHeader className="items-center gap-2 text-center sm:text-center">
            <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
              <Trans>Add members</Trans>
            </DialogTitle>
            <DialogDescription className="sr-only">
              <Trans>
                Invite teammates, share notes across the workspace, and manage
                who has access. Your personal notes stay private. Pending
                invitations are free. New members are billed from when they
                join.
              </Trans>
            </DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              if (trimmedEmail) invite.mutate(trimmedEmail);
            }}
          >
            <Input
              autoFocus
              type="email"
              value={email}
              disabled={invite.isPending}
              aria-label={t`Recipient email`}
              onChange={(event) => {
                setEmail(event.target.value);
                invite.reset();
              }}
              placeholder={t`teammate@company.com`}
            />
            {invite.error ? (
              <p className="text-destructive pt-2 text-center text-xs">
                {invite.error.message}
              </p>
            ) : null}
            <DialogFooter className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-2 sm:justify-normal">
              <GlassDialogCancelButton
                type="button"
                disabled={invite.isPending}
                onClick={() => {
                  setEmail("");
                  invite.reset();
                  setIsInviteDialogOpen(false);
                }}
              >
                <Trans>Cancel</Trans>
              </GlassDialogCancelButton>
              <Button
                type="submit"
                className="h-8 rounded-full px-4 text-xs font-medium shadow-sm"
                disabled={!trimmedEmail || invite.isPending}
              >
                {invite.isPending ? (
                  <CircleNotch className="size-4 animate-spin" />
                ) : null}
                <Trans>Add members</Trans>
              </Button>
            </DialogFooter>
          </form>
        </GlassDialogContent>
      </Dialog>
    </div>
  );
}

function UpcomingCaptureBots({ workspaceId }: { workspaceId: string }) {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const serverUrl = env.VITE_ENTERPRISE_API_URL;
  const accessToken = auth.session?.access_token;
  const upcoming = useQuery({
    queryKey: ["scheduled-captures", workspaceId],
    enabled: Boolean(serverUrl && accessToken),
    retry: false,
    queryFn: () =>
      listScheduledCaptures({
        serverUrl: serverUrl!,
        accessToken: accessToken!,
        workspaceId,
      }),
  });
  const cancel = useMutation({
    mutationFn: (calendarEventId: string) =>
      cancelScheduledCapture({
        serverUrl: serverUrl!,
        accessToken: accessToken!,
        workspaceId,
        calendarEventId,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ["scheduled-captures", workspaceId],
      });
    },
  });

  if (!serverUrl) return null;

  const visible = (upcoming.data ?? []).filter(
    (capture) =>
      capture.status === "pending" || capture.status === "dispatched",
  );

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-medium">
        <Trans>Upcoming bot attendance</Trans>
      </h3>
      <p className="text-muted-foreground text-xs leading-5">
        <Trans>
          Calendar-scheduled capture jobs. Canceling stops the bot from joining.
        </Trans>
      </p>
      {upcoming.isPending ? (
        <p className="text-muted-foreground text-sm">
          <Trans>Loading scheduled captures…</Trans>
        </p>
      ) : upcoming.error ? (
        <p className="text-destructive text-xs">{upcoming.error.message}</p>
      ) : visible.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          <Trans>No upcoming bots.</Trans>
        </p>
      ) : (
        <ul className="flex flex-col">
          {visible.map((capture) => (
            <li
              key={capture.calendarEventId}
              className="flex items-center justify-between gap-3 py-2.5"
            >
              <div className="min-w-0">
                <p className="truncate text-sm">{capture.title}</p>
                <p className="text-muted-foreground text-xs">
                  {new Date(capture.startsAt).toLocaleString()}
                </p>
              </div>
              <Button
                size="sm"
                variant="ghost"
                disabled={cancel.isPending}
                onClick={() => cancel.mutate(capture.calendarEventId)}
              >
                <Trans>Cancel bot</Trans>
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function WorkspacePolicyForm({
  workspaceId,
  policy,
  canConfigureSso,
  canConfigureScim,
  canConfigureRetention,
  onSaved,
}: {
  workspaceId: string;
  policy: WorkspacePolicy;
  canConfigureSso: boolean;
  canConfigureScim: boolean;
  canConfigureRetention: boolean;
  onSaved: () => void;
}) {
  const auth = useAuth();
  const { t } = useLingui();
  const [retention, setRetention] = useState(
    policy.retentionDays?.toString() ?? "",
  );
  const [allowLink, setAllowLink] = useState(
    policy.allowedShareScopes.includes("link"),
  );
  const [allowPublic, setAllowPublic] = useState(
    policy.allowedShareScopes.includes("public"),
  );
  const [requireSso, setRequireSso] = useState(policy.requireSso);
  const [domain, setDomain] = useState("");
  const [scimToken, setScimToken] = useState("");
  const save = useMutation({
    mutationFn: () => {
      const allowedShareScopes: WorkspacePolicy["allowedShareScopes"] = [
        "restricted",
        "workspace",
        ...(allowLink ? (["link"] as const) : []),
        ...(allowPublic ? (["public"] as const) : []),
      ];
      const retentionDays = retention.trim() === "" ? null : Number(retention);
      return setWorkspacePolicy(requireTeamContext(auth), workspaceId, {
        ...policy,
        allowedShareScopes,
        retentionDays:
          canConfigureRetention &&
          retentionDays != null &&
          Number.isFinite(retentionDays)
            ? retentionDays
            : canConfigureRetention
              ? null
              : policy.retentionDays,
        requireSso: canConfigureSso ? requireSso : policy.requireSso,
      });
    },
    onSuccess: onSaved,
  });
  const claimDomain = useMutation({
    mutationFn: (value: string) =>
      claimWorkspaceDomain(requireTeamContext(auth), workspaceId, value),
    onSuccess: onSaved,
  });
  const rotateScim = useMutation({
    mutationFn: () =>
      rotateWorkspaceScimToken(
        requireTeamContext(auth),
        workspaceId,
        domain.trim(),
        scimToken.trim(),
      ),
    onSuccess: () => {
      setScimToken("");
      onSaved();
    },
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-3">
        <h3 className="text-sm font-medium">
          <Trans>Policies</Trans>
        </h3>
        <p className="text-muted-foreground text-xs leading-5">
          <Trans>
            These rules apply to every member. Sharing changes fail closed on
            the server.
          </Trans>
        </p>
      </div>
      <SettingSwitchRow
        title={<Trans>Allow anyone-with-the-link sharing</Trans>}
        checked={allowLink}
        onChange={setAllowLink}
      />
      <SettingSwitchRow
        title={<Trans>Allow public indexing</Trans>}
        checked={allowPublic}
        onChange={setAllowPublic}
      />
      <PlanGate
        plan="enterprise"
        allowed={canConfigureSso || canConfigureScim || canConfigureRetention}
      >
        <div className="flex flex-col gap-6">
          <SettingSwitchRow
            title={<Trans>Require SSO</Trans>}
            description={
              <Trans>
                Members on a claimed email domain must sign in with SSO instead
                of Google, GitHub, or email.
              </Trans>
            }
            checked={requireSso}
            onChange={setRequireSso}
          />
          <label className="flex max-w-sm flex-col gap-1 text-sm">
            <Trans>Retention (days)</Trans>
            <Input
              value={retention}
              onChange={(event) => setRetention(event.target.value)}
              placeholder={t`Keep forever`}
              inputMode="numeric"
              className="bg-card h-9 shadow-none"
            />
          </label>
          <div className="grid gap-6 sm:grid-cols-2">
            <form
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (domain.trim()) claimDomain.mutate(domain.trim());
              }}
            >
              <label className="flex flex-col gap-1 text-sm">
                <Trans>Claim email domain</Trans>
                <Input
                  value={domain}
                  onChange={(event) => setDomain(event.target.value)}
                  placeholder="company.com"
                  className="bg-card h-9 shadow-none"
                />
              </label>
              <Button
                type="submit"
                size="sm"
                variant="outline"
                className="w-fit"
                disabled={!domain.trim() || claimDomain.isPending}
              >
                <Trans>Verify domain</Trans>
              </Button>
            </form>
            <form
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (domain.trim() && scimToken.trim().length >= 32) {
                  rotateScim.mutate();
                }
              }}
            >
              <label className="flex flex-col gap-1 text-sm">
                <Trans>SCIM bearer token</Trans>
                <Input
                  value={scimToken}
                  onChange={(event) => setScimToken(event.target.value)}
                  type="password"
                  autoComplete="off"
                  className="bg-card h-9 shadow-none"
                />
              </label>
              <Button
                type="submit"
                size="sm"
                variant="outline"
                className="w-fit"
                disabled={
                  !domain.trim() ||
                  scimToken.trim().length < 32 ||
                  rotateScim.isPending
                }
              >
                <Trans>Save SCIM token</Trans>
              </Button>
            </form>
          </div>
        </div>
      </PlanGate>
      <Button
        type="button"
        size="sm"
        className="w-fit"
        disabled={save.isPending}
        onClick={() => save.mutate()}
      >
        {save.isPending ? (
          <CircleNotch className="size-4 animate-spin" />
        ) : null}
        <Trans>Save policies</Trans>
      </Button>
      {save.error?.message ? (
        <p className="text-destructive text-xs">{save.error.message}</p>
      ) : null}
    </div>
  );
}

function WorkspaceShareDomainForm({
  workspaceId,
  workspaceShareSlug,
  onSaved,
}: {
  workspaceId: string;
  workspaceShareSlug: string | null;
  onSaved: () => void;
}) {
  const auth = useAuth();
  const inputId = `workspace-share-slug-${workspaceId}`;
  const statusId = `${inputId}-status`;
  const [shareSlug, setShareSlug] = useState(workspaceShareSlug ?? "");
  const inputGroupRef = useSquircleRef<HTMLDivElement>();
  const save = useMutation({
    mutationFn: (value: string) =>
      setWorkspaceShareSlug(requireTeamContext(auth), workspaceId, value),
    onSuccess: (result) => {
      setShareSlug(result.shareSlug);
      onSaved();
    },
  });
  const normalizedShareSlug = shareSlug.trim().toLowerCase();
  const currentShareSlug = save.data?.shareSlug ?? workspaceShareSlug ?? "";
  const isCurrentShareSlug = normalizedShareSlug === currentShareSlug;
  const isValidShareSlug =
    WORKSPACE_SHARE_SLUG_PATTERN.test(normalizedShareSlug);
  const [debouncedShareSlug] = useDebounceValue(normalizedShareSlug, 300);
  const shouldCheckAvailability =
    isValidShareSlug &&
    !isCurrentShareSlug &&
    debouncedShareSlug !== "" &&
    debouncedShareSlug === normalizedShareSlug;
  const availability = useQuery({
    queryKey: [
      "workspace-share-slug-availability",
      workspaceId,
      debouncedShareSlug,
    ],
    queryFn: () =>
      checkWorkspaceShareSlugAvailability(
        requireTeamContext(auth),
        workspaceId,
        debouncedShareSlug,
      ),
    enabled: shouldCheckAvailability,
    retry: false,
  });
  const availabilityStatus =
    normalizedShareSlug === ""
      ? null
      : !isValidShareSlug
        ? "invalid"
        : isCurrentShareSlug
          ? "current"
          : normalizedShareSlug !== debouncedShareSlug ||
              availability.isFetching ||
              (availability.isPending && !availability.isError)
            ? "checking"
            : availability.isError
              ? "error"
              : availability.data;
  const canSave =
    isValidShareSlug &&
    !isCurrentShareSlug &&
    availabilityStatus !== "checking" &&
    availabilityStatus !== "taken" &&
    availabilityStatus !== "invalid";

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (canSave) save.mutate(normalizedShareSlug);
      }}
    >
      <h3 className="text-sm font-medium">
        <Trans>Sharing domain</Trans>
      </h3>
      <p className="text-muted-foreground text-xs">
        <Trans>Use this domain for links shared from this workspace.</Trans>
      </p>
      <label htmlFor={inputId} className="sr-only">
        <Trans>Workspace subdomain</Trans>
      </label>
      <div className="has-[input:focus-visible]:ring-ring rounded-xl has-[input:focus-visible]:ring-1">
        <InputGroup
          ref={inputGroupRef}
          className="bg-card overflow-hidden shadow-none has-[[data-slot=input-group-control]:focus-visible]:ring-0"
        >
          <InputGroupInput
            id={inputId}
            value={shareSlug}
            onChange={(event) => {
              setShareSlug(event.target.value.toLowerCase());
              save.reset();
            }}
            aria-invalid={
              availabilityStatus === "invalid" || availabilityStatus === "taken"
                ? true
                : undefined
            }
            aria-describedby={availabilityStatus ? statusId : undefined}
            placeholder="company"
            minLength={3}
            maxLength={63}
            pattern="[a-z0-9][a-z0-9-]{1,61}[a-z0-9]"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            className="h-full min-w-0"
          />
          <InputGroupAddon
            align="inline-end"
            className="border-input bg-muted h-full shrink-0 border-l px-3 py-0 text-xs font-normal"
          >
            .anarlog.so
          </InputGroupAddon>
        </InputGroup>
      </div>
      {availabilityStatus ? (
        <p
          id={statusId}
          role="status"
          aria-live="polite"
          className={cn([
            "text-xs",
            availabilityStatus === "available" && "text-emerald-600",
            (availabilityStatus === "taken" ||
              availabilityStatus === "invalid") &&
              "text-destructive",
            (availabilityStatus === "checking" ||
              availabilityStatus === "current" ||
              availabilityStatus === "error") &&
              "text-muted-foreground",
          ])}
        >
          {availabilityStatus === "checking" ? (
            <Trans>Checking availability…</Trans>
          ) : availabilityStatus === "available" ? (
            <Trans>Available</Trans>
          ) : availabilityStatus === "taken" ? (
            <Trans>Already taken</Trans>
          ) : availabilityStatus === "invalid" ? (
            <Trans>Use 3–63 lowercase letters, numbers, or hyphens.</Trans>
          ) : availabilityStatus === "current" ? (
            <Trans>Current domain</Trans>
          ) : (
            <Trans>Couldn’t check availability. You can still try Save.</Trans>
          )}
        </p>
      ) : null}
      <Button
        type="submit"
        size="sm"
        variant="outline"
        className="w-fit"
        disabled={!canSave || save.isPending}
      >
        {save.isPending ? (
          <CircleNotch className="size-4 animate-spin" />
        ) : null}
        <Trans>Save subdomain</Trans>
      </Button>
      {save.error?.message ? (
        <p className="text-destructive text-xs">{save.error.message}</p>
      ) : null}
    </form>
  );
}

const WORKSPACE_SHARE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/;

function MemberRow({
  member,
  isViewer,
  viewerRole,
  canManageMembers,
  onRoleChange,
  onRemove,
  onTransfer,
  ownershipPending,
  transferDisabled,
}: {
  member: WorkspaceMember;
  isViewer: boolean;
  viewerRole?: WorkspaceRole;
  canManageMembers: boolean;
  onRoleChange: (role: "admin" | "member") => void;
  onRemove: () => void;
  onTransfer: () => void;
  ownershipPending: boolean;
  transferDisabled: boolean;
}) {
  const { t } = useLingui();
  const isOwner = member.role === "owner";
  // Mirrors the server: owners change any role, admins may only raise a member
  // to admin, and nobody may remove a peer admin or the owner.
  const canEditRole =
    canManageMembers &&
    !isOwner &&
    (viewerRole === "owner" ||
      (viewerRole === "admin" && member.role === "member"));
  const canRemove =
    !isOwner &&
    !isViewer &&
    (viewerRole === "owner" ||
      (viewerRole === "admin" && member.role === "member"));
  const canTransfer = canManageMembers && viewerRole === "owner" && !isOwner;

  return (
    <tr>
      <td className="px-4 py-3">
        <div className="flex items-center gap-3">
          <Avatar
            seed={member.userId}
            label={member.name || member.email}
            imageUrl={member.avatarUrl}
            size={32}
            className="rounded-full"
          />
          <div className="min-w-0">
            <p className="font-medium whitespace-nowrap">
              {member.name || "—"}
            </p>
            {isViewer ? (
              <p className="text-muted-foreground text-xs">
                <Trans>You</Trans>
              </p>
            ) : null}
          </div>
        </div>
      </td>
      <td className="text-muted-foreground px-4 py-3">{member.email}</td>
      <td className="px-4 py-3">
        {!canEditRole ? (
          <span className="text-muted-foreground text-xs">
            {member.role === "owner" ? (
              <Trans>Owner</Trans>
            ) : member.role === "admin" ? (
              <Trans>Admin</Trans>
            ) : (
              <Trans>Member</Trans>
            )}
          </span>
        ) : (
          <Select
            value={member.role}
            onValueChange={(value) => {
              if (value === "owner") onTransfer();
              else onRoleChange(value === "admin" ? "admin" : "member");
            }}
          >
            <SelectTrigger
              className="bg-card h-8 w-28 shadow-none"
              aria-label={t`Permissions for ${member.email}`}
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {canTransfer ? (
                <SelectItem value="owner" disabled={transferDisabled}>
                  <Trans>Owner</Trans>
                </SelectItem>
              ) : null}
              <SelectItem value="admin">
                <Trans>Admin</Trans>
              </SelectItem>
              <SelectItem value="member">
                <Trans>Member</Trans>
              </SelectItem>
            </SelectContent>
          </Select>
        )}
        {ownershipPending ? (
          <span className="text-muted-foreground mt-1 block text-xs">
            <Trans>Pending</Trans>
          </span>
        ) : null}
      </td>
      <td className="px-4 py-3">
        {canRemove ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                size="icon"
                variant="ghost"
                className="size-8"
                aria-label={t`Actions for ${member.email}`}
              >
                <DotsThree className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {canRemove ? (
                <DropdownMenuItem
                  className="text-destructive"
                  onSelect={onRemove}
                >
                  <Trash className="size-4" />
                  <Trans>Remove member</Trans>
                </DropdownMenuItem>
              ) : null}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </td>
    </tr>
  );
}

function TeamSkeleton() {
  return (
    <div className="space-y-2" aria-hidden="true">
      {[0, 1, 2].map((row) => (
        <div key={row} className="bg-muted h-11 animate-pulse rounded-lg" />
      ))}
    </div>
  );
}
