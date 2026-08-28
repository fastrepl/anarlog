import { Trans, useLingui } from "@lingui/react/macro";
import {
  Buildings,
  CalendarBlank,
  ChartBar,
  CircleNotch,
  Crown,
  Globe,
  LockSimple,
  PaperPlaneTilt,
  PencilSimple,
  Plus,
  ShieldCheck,
  Trash,
  UserPlus,
  UsersThree,
  WarningCircle,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { Switch } from "@anlg/ui/components/ui/switch";
import { cn } from "@anlg/utils";

import {
  claimWorkspaceDomain,
  createWorkspace,
  deleteWorkspace,
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
  setWorkspacePolicy,
  setWorkspaceShareSlug,
  transferOwnership,
  type WorkspaceMember,
  type WorkspacePolicy,
  type WorkspaceRole,
} from "./client";
import {
  deliverWorkspaceInvitation,
  getTeamSenderName,
  reportWorkspaceInvitation,
} from "./invitation";
import { MY_WORKSPACES_QUERY_KEY, useMyWorkspacesWithMirror } from "./mirror";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import {
  cancelScheduledCapture,
  listScheduledCaptures,
} from "~/enterprise-capture/client";
import { env } from "~/env";
import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsTeam() {
  const auth = useAuth();
  const billing = useBillingAccess();
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const signedIn = Boolean(auth.supabase && auth.session);

  // Shares the query (and therefore the mirror refresh) with the app-level
  // mount, so opening this page is never what makes sharing scopes appear.
  const workspaces = useMyWorkspacesWithMirror();

  const activeId = selectedId ?? workspaces.data?.[0]?.workspaceId ?? null;
  const activeWorkspace = workspaces.data?.find(
    (workspace) => workspace.workspaceId === activeId,
  );

  const create = useMutation({
    mutationFn: (name: string) =>
      createWorkspace(requireTeamContext(auth), name),
    onSuccess: (result) => {
      setSelectedId(result.workspaceId);
      void queryClient.invalidateQueries({
        queryKey: [MY_WORKSPACES_QUERY_KEY],
      });
    },
  });

  if (!signedIn) {
    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title={<Trans>Team</Trans>} />
        <p className="text-muted-foreground text-sm">
          <Trans>Sign in to create a shared workspace for your team.</Trans>
        </p>
      </div>
    );
  }

  if (!billing.isReady) {
    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title={<Trans>Team</Trans>} />
        <TeamSkeleton />
      </div>
    );
  }

  if (
    !billing.isPro &&
    !workspaces.isPending &&
    (!workspaces.data || workspaces.data.length === 0)
  ) {
    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title={<Trans>Team</Trans>} />
        <div className="border-border/60 bg-card/50 flex max-w-2xl items-center justify-between gap-6 rounded-xl border p-5 shadow-sm">
          <div className="flex items-center gap-4">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <LockSimple className="text-muted-foreground size-5" />
            </div>
            <div>
              <h3 className="text-sm font-medium">
                <Trans>Anarlog Pro required</Trans>
              </h3>
              <p className="text-muted-foreground mt-1 text-xs leading-5">
                <Trans>
                  Invite teammates, share notes across the workspace, and manage
                  who has access. Your personal notes stay private.
                </Trans>
              </p>
            </div>
          </div>
          <Button
            type="button"
            size="sm"
            onClick={billing.upgradeToPro}
            disabled={billing.isUpgradingToPro}
          >
            {billing.isUpgradingToPro ? (
              <CircleNotch className="size-4 animate-spin" />
            ) : null}
            <Trans>Upgrade to Pro</Trans>
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Team</Trans>} />

      {workspaces.isPending ? (
        <TeamSkeleton />
      ) : workspaces.data && workspaces.data.length > 0 ? (
        activeId && (
          <WorkspacePanel
            key={activeId}
            workspaceId={activeId}
            workspaceName={activeWorkspace?.name ?? ""}
            workspaceShareSlug={activeWorkspace?.shareSlug ?? null}
            workspaceRole={activeWorkspace?.role ?? "member"}
            workspaces={workspaces.data}
            hasProAccess={billing.isPro}
            onSelectWorkspace={setSelectedId}
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
        )
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
    <div className="border-border/60 bg-card/50 max-w-2xl rounded-xl border p-5 shadow-sm">
      <div className="flex items-start gap-4">
        <div className="bg-primary/10 text-primary flex size-10 shrink-0 items-center justify-center rounded-xl">
          <Buildings className="size-5" />
        </div>
        <div>
          <h3 className="text-sm font-medium">
            <Trans>Create a shared workspace</Trans>
          </h3>
          <p className="text-muted-foreground mt-1 text-xs leading-5">
            <Trans>
              Invite teammates, share notes across the workspace, and manage who
              has access. Your personal notes stay private.
            </Trans>
          </p>
        </div>
      </div>
      <form
        className="mt-5 flex gap-2"
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
      {error && <p className="text-destructive mt-2 text-xs">{error}</p>}
    </div>
  );
}

function WorkspacePanel({
  workspaceId,
  workspaceName,
  workspaceShareSlug,
  workspaceRole,
  workspaces,
  hasProAccess,
  onSelectWorkspace,
  onWorkspaceRenamed,
  onWorkspaceLeft,
}: {
  workspaceId: string;
  workspaceName: string;
  workspaceShareSlug: string | null;
  workspaceRole: WorkspaceRole;
  workspaces: { workspaceId: string; name: string }[];
  hasProAccess: boolean;
  onSelectWorkspace: (workspaceId: string) => void;
  // Renaming keeps the panel where it is; leaving or deleting must drop the
  // selection because the workspace is gone.
  onWorkspaceRenamed: () => void;
  onWorkspaceLeft: () => void;
}) {
  const auth = useAuth();
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [email, setEmail] = useState("");
  const [isRenaming, setIsRenaming] = useState(false);

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
    enabled:
      hasProAccess && (workspaceRole === "owner" || workspaceRole === "admin"),
  });
  const policy = useQuery({
    queryKey: ["team-policy", workspaceId],
    queryFn: () => getWorkspacePolicy(requireTeamContext(auth), workspaceId),
    retry: false,
    enabled:
      hasProAccess && (workspaceRole === "owner" || workspaceRole === "admin"),
  });

  const refresh = () => {
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
  const canManage =
    hasProAccess && (viewerRole === "owner" || viewerRole === "admin");
  const trimmedEmail = email.trim();
  const actionError =
    invite.error?.message ??
    changeRole.error?.message ??
    remove.error?.message ??
    cancelInvite.error?.message ??
    resendInvite.error?.message ??
    transfer.error?.message ??
    rename.error?.message ??
    leave.error?.message ??
    destroy.error?.message;

  const submitRename = (value: string) => {
    setIsRenaming(false);
    const next = value.trim();
    if (next && next !== workspaceName) rename.mutate(next);
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="border-border/60 bg-card/60 flex items-center gap-4 rounded-xl border p-4 shadow-sm">
        <div className="bg-primary/10 text-primary flex size-10 shrink-0 items-center justify-center rounded-xl">
          <Buildings className="size-5" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            {isRenaming ? (
              <Input
                autoFocus
                defaultValue={workspaceName}
                maxLength={120}
                aria-label={t`Workspace name`}
                className="bg-background h-9 max-w-sm shadow-none"
                onBlur={(event) => submitRename(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    submitRename(event.currentTarget.value);
                  } else if (event.key === "Escape") {
                    setIsRenaming(false);
                  }
                }}
              />
            ) : (
              <>
                <Select value={workspaceId} onValueChange={onSelectWorkspace}>
                  <SelectTrigger className="bg-background h-9 max-w-sm shadow-none">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {workspaces.map((workspace) => (
                      <SelectItem
                        key={workspace.workspaceId}
                        value={workspace.workspaceId}
                      >
                        {workspace.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {canManage && (
                  <Button
                    size="sm"
                    variant="ghost"
                    title={t`Rename workspace`}
                    onClick={() => setIsRenaming(true)}
                  >
                    {rename.isPending ? (
                      <CircleNotch className="size-4 animate-spin" />
                    ) : (
                      <PencilSimple className="size-4" />
                    )}
                  </Button>
                )}
              </>
            )}
          </div>
        </div>
        <span className="border-border bg-background text-muted-foreground shrink-0 rounded-full border px-2.5 py-1 text-xs capitalize">
          {workspaceRole}
        </span>
      </div>

      {actionError && <p className="text-destructive text-xs">{actionError}</p>}

      <div className="grid items-start gap-4 lg:grid-cols-[minmax(0,1.35fr)_minmax(18rem,0.65fr)]">
        <div className="flex min-w-0 flex-col gap-4">
          <section className="border-border/60 bg-card/50 overflow-hidden rounded-xl border shadow-sm">
            <div className="flex items-center justify-between gap-3 p-4">
              <div className="flex items-center gap-3">
                <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-lg">
                  <UsersThree className="text-muted-foreground size-4" />
                </div>
                <h3 className="text-sm font-medium">
                  <Trans>Members</Trans>
                </h3>
              </div>
              {members.data && (
                <span className="bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-xs tabular-nums">
                  {members.data.length + (invitations.data?.length ?? 0)}
                </span>
              )}
            </div>

            <div className="border-border/60 border-t p-4">
              {canManage && (
                <form
                  className="relative mb-4 max-w-sm"
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
                    className="bg-background h-9 rounded-full pr-24 pl-4 shadow-none"
                  />
                  <Button
                    type="submit"
                    size="sm"
                    variant="outline"
                    className="absolute top-1 right-1"
                    disabled={!trimmedEmail || invite.isPending}
                  >
                    {invite.isPending ? (
                      <CircleNotch className="size-4 animate-spin" />
                    ) : (
                      <UserPlus className="size-4" />
                    )}
                    <Trans>Invite</Trans>
                  </Button>
                </form>
              )}

              {members.isPending ? (
                <TeamSkeleton />
              ) : members.isError ? (
                <p className="text-muted-foreground border-border rounded-lg border p-4 text-sm">
                  <Trans>
                    Only workspace admins can see who has access. You are a
                    member of this workspace.
                  </Trans>
                </p>
              ) : (
                <ul className="border-border divide-border divide-y overflow-hidden rounded-lg border">
                  {members.data?.map((member) => (
                    <MemberRow
                      key={member.userId}
                      member={member}
                      isViewer={member.userId === viewerId}
                      viewerRole={canManage ? viewerRole : undefined}
                      onRoleChange={(role) =>
                        changeRole.mutate({ userId: member.userId, role })
                      }
                      onRemove={() => remove.mutate(member.userId)}
                      onTransfer={() => transfer.mutate(member.userId)}
                    />
                  ))}
                  {invitations.data?.map((invitation) => (
                    <li
                      key={invitation.invitationId}
                      className="flex items-center justify-between gap-3 px-3 py-2.5"
                    >
                      <div className="min-w-0">
                        <p className="text-muted-foreground truncate text-sm">
                          {invitation.email}
                        </p>
                        <p className="text-muted-foreground text-xs">
                          <Trans>Invitation pending</Trans>
                        </p>
                      </div>
                      {canManage && (
                        <div className="flex shrink-0 items-center gap-1">
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
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </section>

          {canManage && policy.data && (
            <WorkspacePolicyForm
              workspaceId={workspaceId}
              policy={policy.data}
              onSaved={refresh}
            />
          )}
        </div>

        <div className="flex min-w-0 flex-col gap-4">
          {canManage && (
            <WorkspaceShareDomainForm
              workspaceId={workspaceId}
              workspaceShareSlug={workspaceShareSlug}
              onSaved={() => {
                refresh();
                onWorkspaceRenamed();
              }}
            />
          )}

          {canManage && usage.data && (
            <section className="border-border/60 bg-card/50 rounded-xl border p-4 shadow-sm">
              <div className="flex items-center gap-3">
                <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-lg">
                  <ChartBar className="text-muted-foreground size-4" />
                </div>
                <h3 className="text-sm font-medium">
                  <Trans>Usage</Trans>
                </h3>
              </div>
              <p className="text-muted-foreground mt-3 text-xs leading-5">
                <Trans>
                  Workspace activity from metadata only. Note content stays
                  unreadable on the server.
                </Trans>
              </p>
              <dl className="mt-4 grid grid-cols-2 gap-2 text-sm">
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
                  <div
                    key={label}
                    className="bg-muted/60 rounded-lg px-3 py-2.5"
                  >
                    <dt className="text-muted-foreground text-xs">{label}</dt>
                    <dd className="mt-1 font-medium tabular-nums">{value}</dd>
                  </div>
                ))}
              </dl>
            </section>
          )}

          <UpcomingCaptureBots workspaceId={workspaceId} />
        </div>
      </div>

      <div className="border-destructive/20 bg-destructive/5 flex items-center justify-between gap-4 rounded-xl border px-4 py-3">
        <div className="flex min-w-0 items-center gap-3">
          <WarningCircle className="text-destructive/70 size-4 shrink-0" />
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
        </div>
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
    <section className="border-border/60 bg-card/50 rounded-xl border p-4 shadow-sm">
      <div className="flex items-center gap-3">
        <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-lg">
          <CalendarBlank className="text-muted-foreground size-4" />
        </div>
        <h3 className="text-sm font-medium">
          <Trans>Upcoming bot attendance</Trans>
        </h3>
      </div>
      <p className="text-muted-foreground mt-3 text-xs leading-5">
        <Trans>
          Calendar-scheduled capture jobs. Canceling stops the bot from joining.
        </Trans>
      </p>
      {upcoming.isPending ? (
        <p className="text-muted-foreground mt-3 text-sm">
          <Trans>Loading scheduled captures…</Trans>
        </p>
      ) : upcoming.error ? (
        <p className="text-destructive mt-3 text-xs">
          {upcoming.error.message}
        </p>
      ) : visible.length === 0 ? (
        <p className="text-muted-foreground mt-3 text-sm">
          <Trans>No upcoming bots.</Trans>
        </p>
      ) : (
        <ul className="border-border divide-border mt-3 divide-y overflow-hidden rounded-lg border">
          {visible.map((capture) => (
            <li
              key={capture.calendarEventId}
              className="flex items-center justify-between gap-3 px-3 py-2.5"
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
    </section>
  );
}

function WorkspacePolicyForm({
  workspaceId,
  policy,
  onSaved,
}: {
  workspaceId: string;
  policy: WorkspacePolicy;
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
          retentionDays != null && Number.isFinite(retentionDays)
            ? retentionDays
            : null,
        requireSso,
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
    <section className="border-border/60 bg-card/50 rounded-xl border p-4 shadow-sm">
      <div className="flex items-center gap-3">
        <div className="bg-muted flex size-9 shrink-0 items-center justify-center rounded-lg">
          <ShieldCheck className="text-muted-foreground size-4" />
        </div>
        <h3 className="text-sm font-medium">
          <Trans>Policies</Trans>
        </h3>
      </div>
      <p className="text-muted-foreground mt-3 text-xs leading-5">
        <Trans>
          These rules apply to every member. Sharing changes fail closed on the
          server.
        </Trans>
      </p>
      <div className="mt-4 flex flex-col gap-4">
        <div className="border-border divide-border divide-y overflow-hidden rounded-lg border">
          <label className="flex items-center justify-between gap-4 px-3 py-2.5 text-sm">
            <Trans>Allow anyone-with-the-link sharing</Trans>
            <Switch checked={allowLink} onCheckedChange={setAllowLink} />
          </label>
          <label className="flex items-center justify-between gap-4 px-3 py-2.5 text-sm">
            <Trans>Allow public indexing</Trans>
            <Switch checked={allowPublic} onCheckedChange={setAllowPublic} />
          </label>
          <label className="flex items-center justify-between gap-4 px-3 py-2.5 text-sm">
            <span className="flex min-w-0 flex-col gap-0.5">
              <Trans>Require SSO</Trans>
              <span className="text-muted-foreground text-xs font-normal">
                <Trans>
                  Members on a claimed email domain must sign in with SSO
                  instead of Google, GitHub, or email.
                </Trans>
              </span>
            </span>
            <Switch checked={requireSso} onCheckedChange={setRequireSso} />
          </label>
        </div>
        <label className="flex max-w-sm flex-col gap-1 text-sm">
          <Trans>Retention (days)</Trans>
          <Input
            value={retention}
            onChange={(event) => setRetention(event.target.value)}
            placeholder={t`Keep forever`}
            inputMode="numeric"
            className="bg-background h-9 shadow-none"
          />
        </label>
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
        <div className="border-border/60 grid gap-4 border-t pt-4 sm:grid-cols-2">
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
                className="bg-background h-9 shadow-none"
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
                className="bg-background h-9 shadow-none"
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
    </section>
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
      className="border-border/60 bg-card/50 flex flex-col gap-3 rounded-xl border p-4 shadow-sm"
      onSubmit={(event) => {
        event.preventDefault();
        if (shareSlug.trim()) save.mutate(shareSlug.trim());
      }}
    >
      <div className="flex items-center gap-3">
        <div className="bg-primary/10 text-primary flex size-9 shrink-0 items-center justify-center rounded-lg">
          <Globe className="size-4" />
        </div>
        <h3 className="text-sm font-medium">
          <Trans>Sharing domain</Trans>
        </h3>
      </div>
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
          className="bg-background h-9 min-w-0 rounded-r-none shadow-none"
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
  onRoleChange,
  onRemove,
  onTransfer,
}: {
  member: WorkspaceMember;
  isViewer: boolean;
  viewerRole?: WorkspaceRole;
  onRoleChange: (role: "admin" | "member") => void;
  onRemove: () => void;
  onTransfer: () => void;
}) {
  const { t } = useLingui();
  const isOwner = member.role === "owner";
  // Mirrors the server: owners change any role, admins may only raise a member
  // to admin, and nobody may remove a peer admin or the owner.
  const canEditRole =
    !isOwner &&
    (viewerRole === "owner" ||
      (viewerRole === "admin" && member.role === "member"));
  const canRemove =
    !isOwner &&
    !isViewer &&
    (viewerRole === "owner" ||
      (viewerRole === "admin" && member.role === "member"));
  const canTransfer = viewerRole === "owner" && !isOwner;

  return (
    <li className="flex items-center justify-between gap-3 px-3 py-2.5">
      <div className="min-w-0">
        <p className="truncate text-sm">{member.email}</p>
        {isViewer && (
          <p className="text-muted-foreground text-xs">
            <Trans>You</Trans>
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        {!canEditRole ? (
          <span className={cn(["text-muted-foreground text-xs capitalize"])}>
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
        {canTransfer && (
          <Button
            size="sm"
            variant="ghost"
            onClick={onTransfer}
            title={t`Make owner`}
          >
            <Crown className="size-4" />
          </Button>
        )}
        {canRemove && (
          <Button size="sm" variant="ghost" onClick={onRemove}>
            <Trash className="size-4" />
          </Button>
        )}
      </div>
    </li>
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
