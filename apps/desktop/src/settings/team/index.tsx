import { Trans, useLingui } from "@lingui/react/macro";
import {
  CircleNotch,
  Crown,
  PaperPlaneTilt,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import { Button } from "@anlg/ui/components/ui/button";
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
  claimWorkspaceDomain,
  createWorkspace,
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
  type WorkspaceCapability,
  type WorkspaceMember,
  type WorkspacePolicy,
  type WorkspaceRole,
} from "./client";
import {
  deliverWorkspaceInvitation,
  getTeamSenderName,
  reportWorkspaceInvitation,
} from "./invitation";
import { WorkspaceLogoButton } from "./logo-button";
import { MY_WORKSPACES_QUERY_KEY, useMyWorkspacesWithMirror } from "./mirror";

import { useAuth } from "~/auth";
import {
  cancelScheduledCapture,
  listScheduledCaptures,
} from "~/enterprise-capture/client";
import { env } from "~/env";
import { SettingsPageTitle } from "~/settings/page-title";
import { SettingSwitchRow } from "~/settings/setting-row";
import { buildWebAppUrl } from "~/shared/utils";

export function SettingsTeam() {
  const auth = useAuth();
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const signedIn = Boolean(auth.supabase && auth.session);

  // Shares the query (and therefore the mirror refresh) with the app-level
  // mount, so opening this page is never what makes sharing scopes appear.
  const workspaces = useMyWorkspacesWithMirror();
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

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Teams</Trans>} />

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
            <CreateWorkspaceForm
              onCreate={(name) => create.mutate(name)}
              pending={create.isPending}
              error={create.error?.message}
              placeholder={t`Acme`}
            />
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
        <CreateWorkspaceForm
          onCreate={(name) => create.mutate(name)}
          pending={create.isPending}
          error={create.error?.message}
          placeholder={t`Acme`}
        />
      )}
    </div>
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
  workspaces: { workspaceId: string; name: string }[];
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
              "rounded-full px-3 py-1.5 text-sm transition-colors",
              selected
                ? "bg-muted text-foreground font-medium"
                : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
            ])}
          >
            {workspace.name}
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
  const [email, setEmail] = useState("");
  const [nameDraft, setNameDraft] = useState(workspaceName);
  const [isOpeningBilling, setIsOpeningBilling] = useState(false);
  const isManager = workspaceRole === "owner" || workspaceRole === "admin";

  const access = useQuery({
    queryKey: ["team-access", workspaceId],
    queryFn: () => getWorkspaceAccess(requireTeamContext(auth), workspaceId),
    retry: false,
  });
  const hasCapability = (capability: WorkspaceCapability) =>
    access.data?.capabilities.includes(capability) === true;
  const canManageWorkspace =
    isManager && hasCapability("team.manage_workspace");
  const canManageMembers = isManager && hasCapability("team.manage_members");
  const canManagePolicies = isManager && hasCapability("team.manage_policies");
  const canViewUsage = isManager && hasCapability("team.view_usage");
  const canUseCustomSubdomain =
    isManager && hasCapability("team.custom_subdomain");
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
    onSuccess: refresh,
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
    onSuccess: onWorkspaceLeft,
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
    invite.error?.message ??
    changeRole.error?.message ??
    remove.error?.message ??
    cancelInvite.error?.message ??
    resendInvite.error?.message ??
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
              {hasPaidWorkspacePlan ? (
                <Trans>Team plan</Trans>
              ) : (
                <Trans>Start Team</Trans>
              )}
            </h2>
            <p className="text-muted-foreground mt-1 text-xs leading-5">
              <Trans>
                Pro for every member, shared workspaces, roles, policies, and
                centralized billing. $20 per person monthly or $200 yearly.
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
            {isOpeningBilling ? (
              <CircleNotch className="size-4 animate-spin" />
            ) : null}
            {hasPaidWorkspacePlan ? (
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
            <form
              className="flex items-center gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (trimmedEmail) invite.mutate(trimmedEmail);
              }}
            >
              <Input
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder={t`teammate@company.com`}
                className="bg-card h-9 w-56 shadow-none"
              />
              <Button
                type="submit"
                size="sm"
                disabled={!trimmedEmail || invite.isPending}
              >
                {invite.isPending ? (
                  <CircleNotch className="size-4 animate-spin" />
                ) : null}
                <Trans>Add members</Trans>
              </Button>
            </form>
          ) : null}
        </div>

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
          <table className="w-full text-sm">
            <tbody>
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
                  onTransfer={() => transfer.mutate(member.userId)}
                />
              ))}
              {invitations.data?.map((invitation) => (
                <tr key={invitation.invitationId}>
                  <td className="py-2.5 pr-3">
                    <p className="text-muted-foreground truncate">
                      {invitation.email}
                    </p>
                    <p className="text-muted-foreground text-xs">
                      <Trans>Invitation pending</Trans>
                    </p>
                  </td>
                  <td className="py-2.5 text-right">
                    {isManager ? (
                      <div className="flex items-center justify-end gap-1">
                        {canManageMembers ? (
                          <Button
                            size="sm"
                            variant="ghost"
                            title={t`Resend invitation`}
                            onClick={() =>
                              resendInvite.mutate({
                                email: invitation.email,
                              })
                            }
                            disabled={resendInvite.isPending}
                          >
                            {resendInvite.isPending &&
                            resendInvite.variables?.email ===
                              invitation.email ? (
                              <CircleNotch className="size-4 animate-spin" />
                            ) : (
                              <PaperPlaneTilt className="size-4" />
                            )}
                          </Button>
                        ) : null}
                        <Button
                          size="sm"
                          variant="ghost"
                          title={t`Cancel invitation`}
                          onClick={() =>
                            cancelInvite.mutate(invitation.invitationId)
                          }
                          disabled={cancelInvite.isPending}
                        >
                          <Trash className="size-4" />
                        </Button>
                      </div>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
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

      <div className="flex items-center justify-between gap-4">
        <p className="text-muted-foreground text-xs">
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
            onClick={() => {
              if (confirm(t`Delete ${workspaceName} for everyone?`)) {
                destroy.mutate();
              }
            }}
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
      {canConfigureSso ? (
        <SettingSwitchRow
          title={<Trans>Require SSO</Trans>}
          description={
            <Trans>
              Members on a claimed email domain must sign in with SSO instead of
              Google, GitHub, or email.
            </Trans>
          }
          checked={requireSso}
          onChange={setRequireSso}
        />
      ) : null}
      {canConfigureRetention ? (
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
      ) : null}
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
      {canConfigureSso || canConfigureScim ? (
        <div className="grid gap-6 sm:grid-cols-2">
          {canConfigureSso ? (
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
          ) : null}
          {canConfigureScim ? (
            <form
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (domain.trim() && scimToken.trim().length >= 32) {
                  rotateScim.mutate();
                }
              }}
            >
              {!canConfigureSso ? (
                <label className="flex flex-col gap-1 text-sm">
                  <Trans>Claim email domain</Trans>
                  <Input
                    value={domain}
                    onChange={(event) => setDomain(event.target.value)}
                    placeholder="company.com"
                    className="bg-card h-9 shadow-none"
                  />
                </label>
              ) : null}
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
          ) : null}
        </div>
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
  const [shareSlug, setShareSlug] = useState(workspaceShareSlug ?? "");
  const save = useMutation({
    mutationFn: (value: string) =>
      setWorkspaceShareSlug(requireTeamContext(auth), workspaceId, value),
    onSuccess: (result) => {
      setShareSlug(result.shareSlug);
      onSaved();
    },
  });

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (shareSlug.trim()) save.mutate(shareSlug.trim());
      }}
    >
      <h3 className="text-sm font-medium">
        <Trans>Sharing domain</Trans>
      </h3>
      <p className="text-muted-foreground text-xs">
        <Trans>Use this domain for links shared from this workspace.</Trans>
      </p>
      <label htmlFor={inputId} className="text-xs font-medium">
        <Trans>Workspace subdomain</Trans>
      </label>
      <div className="flex min-w-0 items-center">
        <Input
          id={inputId}
          value={shareSlug}
          onChange={(event) => setShareSlug(event.target.value.toLowerCase())}
          placeholder="company"
          minLength={3}
          maxLength={63}
          pattern="[a-z0-9][a-z0-9-]{1,61}[a-z0-9]"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          className="bg-card h-9 min-w-0 rounded-r-none shadow-none"
        />
        <span className="border-input bg-muted text-muted-foreground flex h-9 shrink-0 items-center rounded-r-md border border-l-0 px-3 text-xs">
          .anarlog.so
        </span>
      </div>
      <Button
        type="submit"
        size="sm"
        variant="outline"
        className="w-fit"
        disabled={!shareSlug.trim() || save.isPending}
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

function MemberRow({
  member,
  isViewer,
  viewerRole,
  canManageMembers,
  onRoleChange,
  onRemove,
  onTransfer,
}: {
  member: WorkspaceMember;
  isViewer: boolean;
  viewerRole?: WorkspaceRole;
  canManageMembers: boolean;
  onRoleChange: (role: "admin" | "member") => void;
  onRemove: () => void;
  onTransfer: () => void;
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
      <td className="py-2.5 pr-3">
        <p className="truncate">{member.email}</p>
        {isViewer ? (
          <p className="text-muted-foreground text-xs">
            <Trans>You</Trans>
          </p>
        ) : null}
      </td>
      <td className="py-2.5 text-right">
        <div className="flex items-center justify-end gap-2">
          {!canEditRole ? (
            <span className="text-muted-foreground text-xs capitalize">
              {member.role}
            </span>
          ) : (
            <Select
              value={member.role}
              onValueChange={(value) =>
                onRoleChange(value === "admin" ? "admin" : "member")
              }
            >
              <SelectTrigger className="bg-card h-8 w-28 shadow-none">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="admin">
                  <Trans>Admin</Trans>
                </SelectItem>
                <SelectItem value="member">
                  <Trans>Member</Trans>
                </SelectItem>
              </SelectContent>
            </Select>
          )}
          {canTransfer ? (
            <Button
              size="sm"
              variant="ghost"
              onClick={onTransfer}
              title={t`Make owner`}
            >
              <Crown className="size-4" />
            </Button>
          ) : null}
          {canRemove ? (
            <Button size="sm" variant="ghost" onClick={onRemove}>
              <Trash className="size-4" />
            </Button>
          ) : null}
        </div>
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
