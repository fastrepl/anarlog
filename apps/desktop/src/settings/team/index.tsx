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
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
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
      <div {...stylex.props(styles.page)}>
        <SettingsPageTitle title={<Trans>Team</Trans>} />
        <p {...stylex.props(styles.muted)}>
          <Trans>Sign in to create a shared workspace for your team.</Trans>
        </p>
      </div>
    );
  }

  if (!billing.isReady) {
    return (
      <div {...stylex.props(styles.page)}>
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
      <div {...stylex.props(styles.page)}>
        <SettingsPageTitle title={<Trans>Team</Trans>} />
        <div {...stylex.props(styles.proUpsell)}>
          <div {...stylex.props(styles.row)}>
            <div {...stylex.props(styles.largeMutedIconFrame)}>
              <LockSimple {...stylex.props(styles.largeMutedIcon)} />
            </div>
            <div>
              <h3 {...stylex.props(styles.heading)}>
                <Trans>Anarlog Pro required</Trans>
              </h3>
              <p {...stylex.props(styles.description)}>
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
              <CircleNotch {...stylex.props(styles.icon, styles.spinning)} />
            ) : null}
            <Trans>Upgrade to Pro</Trans>
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.page)}>
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
    <div {...stylex.props(styles.createCard)}>
      <div {...stylex.props(styles.createHeader)}>
        <div {...stylex.props(styles.largePrimaryIconFrame)}>
          <Buildings {...stylex.props(styles.largeIcon)} />
        </div>
        <div>
          <h3 {...stylex.props(styles.heading)}>
            <Trans>Create a shared workspace</Trans>
          </h3>
          <p {...stylex.props(styles.description)}>
            <Trans>
              Invite teammates, share notes across the workspace, and manage who
              has access. Your personal notes stay private.
            </Trans>
          </p>
        </div>
      </div>
      <form
        {...stylex.props(styles.createForm)}
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
          sx={styles.createInput}
        />
        <Button type="submit" size="sm" disabled={!trimmed || pending}>
          {pending ? (
            <CircleNotch {...stylex.props(styles.icon, styles.spinning)} />
          ) : (
            <Plus {...stylex.props(styles.icon)} />
          )}
          <Trans>Create</Trans>
        </Button>
      </form>
      {error && <p {...stylex.props(styles.errorWithMargin)}>{error}</p>}
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
    <div {...stylex.props(styles.workspace)}>
      <div {...stylex.props(styles.workspaceHeader)}>
        <div {...stylex.props(styles.largePrimaryIconFrame)}>
          <Buildings {...stylex.props(styles.largeIcon)} />
        </div>
        <div {...stylex.props(styles.flexible)}>
          <div {...stylex.props(styles.rowSmall)}>
            {isRenaming ? (
              <Input
                autoFocus
                defaultValue={workspaceName}
                maxLength={120}
                aria-label={t`Workspace name`}
                sx={styles.workspaceControl}
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
                  <SelectTrigger sx={styles.workspaceControl}>
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
                      <CircleNotch
                        {...stylex.props(styles.icon, styles.spinning)}
                      />
                    ) : (
                      <PencilSimple {...stylex.props(styles.icon)} />
                    )}
                  </Button>
                )}
              </>
            )}
          </div>
        </div>
        <span {...stylex.props(styles.roleBadge)}>{workspaceRole}</span>
      </div>

      {actionError && <p {...stylex.props(styles.error)}>{actionError}</p>}

      <div {...stylex.props(styles.workspaceGrid)}>
        <div {...stylex.props(styles.column)}>
          <section {...stylex.props(styles.membersCard)}>
            <div {...stylex.props(styles.cardHeader)}>
              <div {...stylex.props(styles.cardTitleRow)}>
                <div {...stylex.props(styles.mutedIconFrame)}>
                  <UsersThree {...stylex.props(styles.mutedIcon)} />
                </div>
                <h3 {...stylex.props(styles.heading)}>
                  <Trans>Members</Trans>
                </h3>
              </div>
              {members.data && (
                <span {...stylex.props(styles.countBadge)}>
                  {members.data.length + (invitations.data?.length ?? 0)}
                </span>
              )}
            </div>

            <div {...stylex.props(styles.membersBody)}>
              {canManage && (
                <form
                  {...stylex.props(styles.inviteForm)}
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
                    sx={styles.inviteInput}
                  />
                  <Button
                    type="submit"
                    size="sm"
                    variant="outline"
                    sx={styles.inviteButton}
                    disabled={!trimmedEmail || invite.isPending}
                  >
                    {invite.isPending ? (
                      <CircleNotch
                        {...stylex.props(styles.icon, styles.spinning)}
                      />
                    ) : (
                      <UserPlus {...stylex.props(styles.icon)} />
                    )}
                    <Trans>Invite</Trans>
                  </Button>
                </form>
              )}

              {members.isPending ? (
                <TeamSkeleton />
              ) : members.isError ? (
                <p {...stylex.props(styles.membersError)}>
                  <Trans>
                    Only workspace admins can see who has access. You are a
                    member of this workspace.
                  </Trans>
                </p>
              ) : (
                <ul {...stylex.props(styles.memberList)}>
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
                      {...stylex.props(styles.memberRow)}
                    >
                      <div {...stylex.props(styles.minWidth)}>
                        <p {...stylex.props(styles.invitationEmail)}>
                          {invitation.email}
                        </p>
                        <p {...stylex.props(styles.mutedSmall)}>
                          <Trans>Invitation pending</Trans>
                        </p>
                      </div>
                      {canManage && (
                        <div {...stylex.props(styles.memberActions)}>
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
                              <CircleNotch
                                {...stylex.props(styles.icon, styles.spinning)}
                              />
                            ) : (
                              <PaperPlaneTilt {...stylex.props(styles.icon)} />
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
                            <Trash {...stylex.props(styles.icon)} />
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

        <div {...stylex.props(styles.column)}>
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
            <section {...stylex.props(styles.card)}>
              <div {...stylex.props(styles.cardTitleRow)}>
                <div {...stylex.props(styles.mutedIconFrame)}>
                  <ChartBar {...stylex.props(styles.mutedIcon)} />
                </div>
                <h3 {...stylex.props(styles.heading)}>
                  <Trans>Usage</Trans>
                </h3>
              </div>
              <p {...stylex.props(styles.cardDescription)}>
                <Trans>
                  Workspace activity from metadata only. Note content stays
                  unreadable on the server.
                </Trans>
              </p>
              <dl {...stylex.props(styles.usageGrid)}>
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
                  <div key={label} {...stylex.props(styles.usageItem)}>
                    <dt {...stylex.props(styles.mutedSmall)}>{label}</dt>
                    <dd {...stylex.props(styles.usageValue)}>{value}</dd>
                  </div>
                ))}
              </dl>
            </section>
          )}

          <UpcomingCaptureBots workspaceId={workspaceId} />
        </div>
      </div>

      <div {...stylex.props(styles.dangerZone)}>
        <div {...stylex.props(styles.dangerCopy)}>
          <WarningCircle {...stylex.props(styles.dangerIcon)} />
          <p {...stylex.props(styles.mutedSmall)}>
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
            sx={styles.noShrink}
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
            sx={styles.noShrink}
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
    <section {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.cardTitleRow)}>
        <div {...stylex.props(styles.mutedIconFrame)}>
          <CalendarBlank {...stylex.props(styles.mutedIcon)} />
        </div>
        <h3 {...stylex.props(styles.heading)}>
          <Trans>Upcoming bot attendance</Trans>
        </h3>
      </div>
      <p {...stylex.props(styles.cardDescription)}>
        <Trans>
          Calendar-scheduled capture jobs. Canceling stops the bot from joining.
        </Trans>
      </p>
      {upcoming.isPending ? (
        <p {...stylex.props(styles.cardStatus)}>
          <Trans>Loading scheduled captures…</Trans>
        </p>
      ) : upcoming.error ? (
        <p {...stylex.props(styles.cardError)}>{upcoming.error.message}</p>
      ) : visible.length === 0 ? (
        <p {...stylex.props(styles.cardStatus)}>
          <Trans>No upcoming bots.</Trans>
        </p>
      ) : (
        <ul {...stylex.props(styles.captureList)}>
          {visible.map((capture) => (
            <li
              key={capture.calendarEventId}
              {...stylex.props(styles.memberRow)}
            >
              <div {...stylex.props(styles.minWidth)}>
                <p {...stylex.props(styles.truncated)}>{capture.title}</p>
                <p {...stylex.props(styles.mutedSmall)}>
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
    <section {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.cardTitleRow)}>
        <div {...stylex.props(styles.mutedIconFrame)}>
          <ShieldCheck {...stylex.props(styles.mutedIcon)} />
        </div>
        <h3 {...stylex.props(styles.heading)}>
          <Trans>Policies</Trans>
        </h3>
      </div>
      <p {...stylex.props(styles.cardDescription)}>
        <Trans>
          These rules apply to every member. Sharing changes fail closed on the
          server.
        </Trans>
      </p>
      <div {...stylex.props(styles.policyContent)}>
        <div {...stylex.props(styles.policyOptions)}>
          <label {...stylex.props(styles.policyOption)}>
            <Trans>Allow anyone-with-the-link sharing</Trans>
            <Switch checked={allowLink} onCheckedChange={setAllowLink} />
          </label>
          <label {...stylex.props(styles.policyOption)}>
            <Trans>Allow public indexing</Trans>
            <Switch checked={allowPublic} onCheckedChange={setAllowPublic} />
          </label>
          <label {...stylex.props(styles.policyOption)}>
            <span {...stylex.props(styles.policyOptionCopy)}>
              <Trans>Require SSO</Trans>
              <span {...stylex.props(styles.policyOptionDescription)}>
                <Trans>
                  Members on a claimed email domain must sign in with SSO
                  instead of Google, GitHub, or email.
                </Trans>
              </span>
            </span>
            <Switch checked={requireSso} onCheckedChange={setRequireSso} />
          </label>
        </div>
        <label {...stylex.props(styles.fieldLabel)}>
          <Trans>Retention (days)</Trans>
          <Input
            value={retention}
            onChange={(event) => setRetention(event.target.value)}
            placeholder={t`Keep forever`}
            inputMode="numeric"
            sx={styles.fieldInput}
          />
        </label>
        <Button
          type="button"
          size="sm"
          sx={styles.fitButton}
          disabled={save.isPending}
          onClick={() => save.mutate()}
        >
          {save.isPending ? (
            <CircleNotch {...stylex.props(styles.icon, styles.spinning)} />
          ) : null}
          <Trans>Save policies</Trans>
        </Button>
        {save.error?.message ? (
          <p {...stylex.props(styles.error)}>{save.error.message}</p>
        ) : null}
        <div {...stylex.props(styles.policyForms)}>
          <form
            {...stylex.props(styles.formStack)}
            onSubmit={(event) => {
              event.preventDefault();
              if (domain.trim()) claimDomain.mutate(domain.trim());
            }}
          >
            <label {...stylex.props(styles.fieldLabel)}>
              <Trans>Claim email domain</Trans>
              <Input
                value={domain}
                onChange={(event) => setDomain(event.target.value)}
                placeholder="company.com"
                sx={styles.fieldInput}
              />
            </label>
            <Button
              type="submit"
              size="sm"
              variant="outline"
              sx={styles.fitButton}
              disabled={!domain.trim() || claimDomain.isPending}
            >
              <Trans>Verify domain</Trans>
            </Button>
          </form>
          <form
            {...stylex.props(styles.formStack)}
            onSubmit={(event) => {
              event.preventDefault();
              if (domain.trim() && scimToken.trim().length >= 32) {
                rotateScim.mutate();
              }
            }}
          >
            <label {...stylex.props(styles.fieldLabel)}>
              <Trans>SCIM bearer token</Trans>
              <Input
                value={scimToken}
                onChange={(event) => setScimToken(event.target.value)}
                type="password"
                autoComplete="off"
                sx={styles.fieldInput}
              />
            </label>
            <Button
              type="submit"
              size="sm"
              variant="outline"
              sx={styles.fitButton}
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
      {...stylex.props(styles.shareDomainForm)}
      onSubmit={(event) => {
        event.preventDefault();
        if (shareSlug.trim()) save.mutate(shareSlug.trim());
      }}
    >
      <div {...stylex.props(styles.cardTitleRow)}>
        <div {...stylex.props(styles.primaryIconFrame)}>
          <Globe {...stylex.props(styles.icon)} />
        </div>
        <h3 {...stylex.props(styles.heading)}>
          <Trans>Sharing domain</Trans>
        </h3>
      </div>
      <p {...stylex.props(styles.mutedSmall)}>
        <Trans>Use this domain for links shared from this workspace.</Trans>
      </p>
      <label htmlFor={inputId} {...stylex.props(styles.smallLabel)}>
        <Trans>Workspace subdomain</Trans>
      </label>
      <div {...stylex.props(styles.slugRow)}>
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
          sx={styles.slugInput}
        />
        <span {...stylex.props(styles.slugSuffix)}>.anarlog.so</span>
      </div>
      <Button
        type="submit"
        size="sm"
        variant="outline"
        sx={styles.fitButton}
        disabled={!shareSlug.trim() || save.isPending}
      >
        {save.isPending ? (
          <CircleNotch {...stylex.props(styles.icon, styles.spinning)} />
        ) : null}
        <Trans>Save subdomain</Trans>
      </Button>
      {save.error?.message ? (
        <p {...stylex.props(styles.error)}>{save.error.message}</p>
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
    <li {...stylex.props(styles.memberRow)}>
      <div {...stylex.props(styles.minWidth)}>
        <p {...stylex.props(styles.truncated)}>{member.email}</p>
        {isViewer && (
          <p {...stylex.props(styles.mutedSmall)}>
            <Trans>You</Trans>
          </p>
        )}
      </div>
      <div {...stylex.props(styles.memberRoleActions)}>
        {!canEditRole ? (
          <span {...stylex.props(styles.roleText)}>{member.role}</span>
        ) : (
          <Select
            value={member.role}
            onValueChange={(value) =>
              onRoleChange(value === "admin" ? "admin" : "member")
            }
          >
            <SelectTrigger sx={styles.roleSelect}>
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
            <Crown {...stylex.props(styles.icon)} />
          </Button>
        )}
        {canRemove && (
          <Button size="sm" variant="ghost" onClick={onRemove}>
            <Trash {...stylex.props(styles.icon)} />
          </Button>
        )}
      </div>
    </li>
  );
}

function TeamSkeleton() {
  return (
    <div {...stylex.props(styles.skeleton)} aria-hidden="true">
      {[0, 1, 2].map((row) => (
        <div key={row} {...stylex.props(styles.skeletonRow)} />
      ))}
    </div>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const pulse = stylex.keyframes({
  "50%": {
    opacity: 0.5,
  },
});

const styles = stylex.create({
  captureList: {
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    marginTop: "0.75rem",
    overflow: "hidden",
  },
  card: {
    backgroundColor: `color-mix(in srgb, ${colors.card} 50%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    padding: "1rem",
  },
  cardDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
    marginTop: "0.75rem",
  },
  cardError: {
    color: colors.destructive,
    fontSize: "0.75rem",
    marginTop: "0.75rem",
  },
  cardHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    padding: "1rem",
  },
  cardStatus: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    marginTop: "0.75rem",
  },
  cardTitleRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
  },
  column: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    minWidth: 0,
  },
  countBadge: {
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontVariantNumeric: "tabular-nums",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
  },
  createCard: {
    backgroundColor: `color-mix(in srgb, ${colors.card} 50%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    maxWidth: "42rem",
    padding: "1.25rem",
  },
  createForm: {
    display: "flex",
    gap: "0.5rem",
    marginTop: "1.25rem",
  },
  createHeader: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1rem",
  },
  createInput: {
    backgroundColor: colors.card,
    boxShadow: "none",
    height: "2.25rem",
    maxWidth: "20rem",
  },
  dangerCopy: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    minWidth: 0,
  },
  dangerIcon: {
    color: `color-mix(in srgb, ${colors.destructive} 70%, transparent)`,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  dangerZone: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.destructive} 5%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.destructive} 20%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.75rem",
  },
  errorWithMargin: {
    color: colors.destructive,
    fontSize: "0.75rem",
    marginTop: "0.5rem",
  },
  fieldInput: {
    backgroundColor: colors.background,
    boxShadow: "none",
    height: "2.25rem",
  },
  fieldLabel: {
    display: "flex",
    flexDirection: "column",
    fontSize: "0.875rem",
    gap: "0.25rem",
    maxWidth: "24rem",
  },
  fitButton: {
    width: "fit-content",
  },
  flexible: {
    flex: "1",
    minWidth: 0,
  },
  formStack: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  heading: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  invitationEmail: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  inviteButton: {
    position: "absolute",
    right: "0.25rem",
    top: "0.25rem",
  },
  inviteForm: {
    marginBottom: "1rem",
    maxWidth: "24rem",
    position: "relative",
  },
  inviteInput: {
    backgroundColor: colors.background,
    borderRadius: radii.full,
    boxShadow: "none",
    height: "2.25rem",
    paddingLeft: "1rem",
    paddingRight: "6rem",
  },
  largeIcon: {
    height: "1.25rem",
    width: "1.25rem",
  },
  largeMutedIcon: {
    color: colors.mutedForeground,
    height: "1.25rem",
    width: "1.25rem",
  },
  largeMutedIconFrame: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.xl,
    display: "flex",
    flexShrink: 0,
    height: "2.5rem",
    justifyContent: "center",
    width: "2.5rem",
  },
  largePrimaryIconFrame: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.primary} 10%, transparent)`,
    borderRadius: radii.xl,
    color: colors.primary,
    display: "flex",
    flexShrink: 0,
    height: "2.5rem",
    justifyContent: "center",
    width: "2.5rem",
  },
  memberActions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.25rem",
  },
  memberList: {
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  memberRoleActions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.5rem",
  },
  memberRow: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
  },
  membersBody: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    padding: "1rem",
  },
  membersCard: {
    backgroundColor: `color-mix(in srgb, ${colors.card} 50%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    overflow: "hidden",
  },
  membersError: {
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    padding: "1rem",
  },
  minWidth: {
    minWidth: 0,
  },
  muted: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  mutedIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  mutedIconFrame: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.lg,
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    justifyContent: "center",
    width: "2.25rem",
  },
  mutedSmall: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  noShrink: {
    flexShrink: 0,
  },
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
  policyContent: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    marginTop: "1rem",
  },
  policyForms: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "grid",
    gap: "1rem",
    gridTemplateColumns: {
      default: "minmax(0, 1fr)",
      "@media (min-width: 40rem)": "repeat(2, minmax(0, 1fr))",
    },
    paddingTop: "1rem",
  },
  policyOption: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    fontSize: "0.875rem",
    gap: "1rem",
    justifyContent: "space-between",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
  },
  policyOptionCopy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
    minWidth: 0,
  },
  policyOptionDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 400,
  },
  policyOptions: {
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  primaryIconFrame: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.primary} 10%, transparent)`,
    borderRadius: radii.lg,
    color: colors.primary,
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    justifyContent: "center",
    width: "2.25rem",
  },
  proUpsell: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.card} 50%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    display: "flex",
    gap: "1.5rem",
    justifyContent: "space-between",
    maxWidth: "42rem",
    padding: "1.25rem",
  },
  roleBadge: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.625rem",
    textTransform: "capitalize",
  },
  roleSelect: {
    backgroundColor: colors.card,
    boxShadow: "none",
    height: "2rem",
    width: "7rem",
  },
  roleText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    textTransform: "capitalize",
  },
  row: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
  },
  rowSmall: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  shareDomainForm: {
    backgroundColor: `color-mix(in srgb, ${colors.card} 50%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    padding: "1rem",
  },
  skeleton: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  skeletonRow: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    backgroundColor: colors.muted,
    borderRadius: radii.lg,
    height: "2.75rem",
  },
  slugInput: {
    backgroundColor: colors.background,
    borderBottomRightRadius: 0,
    borderTopRightRadius: 0,
    boxShadow: "none",
    height: "2.25rem",
    minWidth: 0,
  },
  slugRow: {
    alignItems: "center",
    display: "flex",
    minWidth: 0,
  },
  slugSuffix: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderBottomColor: colors.input,
    borderBottomRightRadius: radii.md,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderRightColor: colors.input,
    borderRightStyle: "solid",
    borderRightWidth: "1px",
    borderTopColor: colors.input,
    borderTopRightRadius: radii.md,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    flexShrink: 0,
    fontSize: "0.75rem",
    height: "2.25rem",
    paddingInline: "0.75rem",
  },
  smallLabel: {
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  truncated: {
    fontSize: "0.875rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  usageGrid: {
    display: "grid",
    fontSize: "0.875rem",
    gap: "0.5rem",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    marginTop: "1rem",
  },
  usageItem: {
    backgroundColor: `color-mix(in srgb, ${colors.muted} 60%, transparent)`,
    borderRadius: radii.lg,
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
  },
  usageValue: {
    fontVariantNumeric: "tabular-nums",
    fontWeight: 500,
    marginTop: "0.25rem",
  },
  workspace: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  workspaceControl: {
    backgroundColor: colors.background,
    boxShadow: "none",
    height: "2.25rem",
    maxWidth: "24rem",
  },
  workspaceGrid: {
    alignItems: "start",
    display: "grid",
    gap: "1rem",
    gridTemplateColumns: {
      default: "minmax(0, 1fr)",
      "@media (min-width: 64rem)": "minmax(0, 1.35fr) minmax(18rem, 0.65fr)",
    },
  },
  workspaceHeader: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.card} 60%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    display: "flex",
    gap: "1rem",
    padding: "1rem",
  },
});
