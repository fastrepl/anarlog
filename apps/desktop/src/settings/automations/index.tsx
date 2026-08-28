import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowRight,
  DotsThree,
  Eye,
  FloppyDisk,
  Lightning,
  Play,
  Sparkle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";
import { formatDistanceToNow } from "@anlg/utils";

import {
  AutomationLastRunLine,
  LinearIssuesConfig,
  MarkdownExportConfig,
  NotionUpdateConfig,
  SlackRecapConfig,
} from "./starter-config";
import { useSaveWorkflow, WorkflowBuilder } from "./workflow-builder";

import { useBillingAccess } from "~/auth/billing-context";
import {
  useDeleteChatAutomation,
  useDeleteWorkflow,
  useRemoveStarterDraft,
} from "~/automations/actions";
import { parseAutomationTargetRef } from "~/automations/engine";
import {
  useAutomationSelection,
  useEffectiveAutomationSelection,
} from "~/automations/selection";
import {
  STARTER_AUTOMATIONS,
  type StarterId,
  useStarterAutomations,
} from "~/automations/starters";
import {
  type AutomationWorkflow,
  createEmptyWorkflow,
  isWorkflowReady,
  parseAutomationWorkflows,
  saveAutomationWorkflows,
  useAutomationWorkflows,
} from "~/automations/workflows";
import { useChatGroup } from "~/chat/store/queries";
import { SettingsHydrationBoundary } from "~/settings/hydration-boundary";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  getStoredSettingValues,
  setSettingValue,
  setSettingValues,
  useStoredSettingValues,
} from "~/settings/queries";
import type { SettingValues } from "~/settings/schema";
import { StandardContentWrapper } from "~/shared/main";

export function TabContentAutomations() {
  return (
    <StandardContentWrapper>
      <SettingsHydrationBoundary>
        <div {...stylex.props(styles.page)}>
          <AutomationsContent />
        </div>
      </SettingsHydrationBoundary>
    </StandardContentWrapper>
  );
}

export function AutomationsContent() {
  const selection = useEffectiveAutomationSelection();

  if (selection?.kind === "starter") {
    return (
      <StarterAutomationDetails
        key={selection.starterId}
        starterId={selection.starterId}
      />
    );
  }

  if (selection?.kind === "chat") {
    return <ChatAutomationDetails groupId={selection.groupId} />;
  }

  if (selection?.kind === "draft") {
    return <DraftAutomationDetails draftId={selection.draftId} />;
  }

  if (selection?.kind === "workflow") {
    return <PersistedWorkflowDetails workflowId={selection.workflowId} />;
  }

  return <AutomationsOverview />;
}

function AutomationsOverview() {
  return (
    <div {...mergeStyleXProps(styles.scroller, "scroll-fade-y")}>
      <div {...stylex.props(styles.content)}>
        <div {...stylex.props(styles.titleBlock)}>
          <SettingsPageTitle title={<Trans>Automations</Trans>} />
          <p {...stylex.props(styles.description)}>
            <Trans>
              Automate what happens before, during, or after meetings based on
              the conditions you choose.
            </Trans>
          </p>
        </div>

        <section {...stylex.props(styles.emptyState)}>
          <span {...stylex.props(styles.emptyIconFrame)}>
            <Lightning {...stylex.props(styles.muted)} size={20} />
          </span>
          <h3 {...stylex.props(styles.emptyTitle)}>
            <Trans>No automation draft yet</Trans>
          </h3>
          <p {...stylex.props(styles.emptyDescription)}>
            <Trans>
              Choose a starter from the sidebar, or create a workflow and add
              steps like Zapier.
            </Trans>
          </p>
        </section>
      </div>
    </div>
  );
}

function DraftAutomationDetails({ draftId }: { draftId: string }) {
  const removeDraft = useAutomationSelection((state) => state.removeDraft);
  const workflow = useEnsuredWorkflow({ id: draftId });

  return (
    <CustomWorkflowDetails
      workflow={workflow}
      title={workflow.title.trim() || <Trans>Untitled automation</Trans>}
      description={
        <Trans>
          Add a trigger and actions. Chat on the right can help you refine the
          workflow.
        </Trans>
      }
      onDelete={() => removeDraft(draftId)}
    />
  );
}

function AutomationDetailHeader({
  icon,
  title,
  actions,
}: {
  icon: React.ReactNode;
  title: React.ReactNode;
  actions: React.ReactNode;
}) {
  return (
    <header {...stylex.props(styles.detailHeader)}>
      <div {...stylex.props(styles.detailTitleRow)}>
        <span {...stylex.props(styles.detailIcon)}>{icon}</span>
        <h2 {...stylex.props(styles.detailTitle)}>{title}</h2>
      </div>
      {actions}
    </header>
  );
}

function AutomationDetailsLayout({
  icon,
  title,
  description,
  actions,
  children,
}: {
  icon: React.ReactNode;
  title: React.ReactNode;
  description: React.ReactNode;
  actions: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div {...stylex.props(styles.detailLayout)}>
      <AutomationDetailHeader icon={icon} title={title} actions={actions} />
      <div {...mergeStyleXProps(styles.detailScroller, "scroll-fade-y")}>
        <div {...stylex.props(styles.content)}>
          {description && (
            <p {...stylex.props(styles.description)}>{description}</p>
          )}
          {children}
        </div>
      </div>
    </div>
  );
}

function AutomationActionsMenu({
  actionLabel,
  onAction,
}: {
  actionLabel: React.ReactNode;
  onAction: () => void;
}) {
  const { t } = useLingui();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          sx={styles.menuTrigger}
          aria-label={t`Automation actions`}
        >
          <DotsThree {...stylex.props(styles.icon)} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end">
        <AppFloatingPanel sx={styles.menuPanel}>
          <DropdownMenuItem onClick={onAction} sx={styles.pointer}>
            {actionLabel}
          </DropdownMenuItem>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function ChatAutomationDetails({ groupId }: { groupId: string }) {
  const { t } = useLingui();
  const group = useChatGroup(groupId, "automations");
  const deleteChatAutomation = useDeleteChatAutomation();
  const createdAt = group?.createdAt
    ? formatDistanceToNow(new Date(group.createdAt), { addSuffix: true })
    : "";
  const workflow = useEnsuredWorkflow({
    chatGroupId: groupId,
    title: group?.title.trim() || undefined,
  });

  return (
    <CustomWorkflowDetails
      workflow={workflow}
      title={group?.title.trim() || workflow.title || t`Untitled automation`}
      description={createdAt ? <Trans>Created {createdAt}</Trans> : null}
      onDelete={() => deleteChatAutomation.mutate(groupId)}
    />
  );
}

function PersistedWorkflowDetails({ workflowId }: { workflowId: string }) {
  const { t } = useLingui();
  const deleteWorkflow = useDeleteWorkflow();
  const workflow = useEnsuredWorkflow({ id: workflowId });

  return (
    <CustomWorkflowDetails
      workflow={workflow}
      title={workflow.title.trim() || t`Untitled automation`}
      description={
        <Trans>
          Add a trigger and actions. Chat on the right can help you refine the
          workflow.
        </Trans>
      }
      onDelete={() => deleteWorkflow.mutate(workflowId)}
    />
  );
}

function CustomWorkflowDetails({
  workflow,
  title,
  description,
  onDelete,
}: {
  workflow: AutomationWorkflow;
  title: React.ReactNode;
  description: React.ReactNode;
  onDelete: () => void;
}) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const workflows = useAutomationWorkflows();
  const saveWorkflow = useSaveWorkflow();

  const persist = (next: AutomationWorkflow) => {
    saveWorkflow.mutate({ workflows, next });
  };

  const handleEnable = (enabled: boolean) => {
    if (enabled && !billing.isPro) {
      billing.upgradeToPro();
      return;
    }
    persist({ ...workflow, enabled });
  };

  return (
    <AutomationDetailsLayout
      icon={
        <Lightning
          {...stylex.props(styles.workflowIcon)}
          size={16}
          weight="fill"
        />
      }
      title={title}
      description={description}
      actions={
        <div {...stylex.props(styles.headerActions)}>
          {workflow.enabled ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => handleEnable(false)}
              disabled={!billing.isReady || saveWorkflow.isPending}
            >
              <Trans>Disable</Trans>
            </Button>
          ) : (
            <Button
              type="button"
              size="sm"
              onClick={() => handleEnable(true)}
              disabled={
                !billing.isReady ||
                saveWorkflow.isPending ||
                (billing.isPro && !isWorkflowReady(workflow))
              }
              title={
                billing.isPro && !isWorkflowReady(workflow)
                  ? t`Add and configure at least one action first.`
                  : undefined
              }
            >
              <Lightning size={14} weight="fill" />
              {billing.isPro ? (
                <Trans>Save &amp; enable</Trans>
              ) : (
                <Trans>Upgrade to enable</Trans>
              )}
            </Button>
          )}
          <AutomationActionsMenu
            actionLabel={<Trans>Delete automation</Trans>}
            onAction={onDelete}
          />
        </div>
      }
    >
      <WorkflowBuilder workflow={workflow} onChange={persist} />
    </AutomationDetailsLayout>
  );
}

function useEnsuredWorkflow({
  id,
  chatGroupId,
  title,
}: {
  id?: string;
  chatGroupId?: string;
  title?: string;
}): AutomationWorkflow {
  const workflows = useAutomationWorkflows();
  const existing =
    (id ? workflows.find((workflow) => workflow.id === id) : undefined) ??
    (chatGroupId
      ? workflows.find((workflow) => workflow.chatGroupId === chatGroupId)
      : undefined);
  const fallback = useMemo(
    () =>
      createEmptyWorkflow({
        id,
        chatGroupId: chatGroupId ?? null,
        title,
      }),
    [chatGroupId, id, title],
  );

  useEffect(() => {
    if (existing) {
      return;
    }

    void (async () => {
      const stored = await getStoredSettingValues();
      const current = parseAutomationWorkflows(
        stored.values.automation_workflows,
      );
      if (
        current.some(
          (workflow) =>
            workflow.id === fallback.id ||
            (chatGroupId && workflow.chatGroupId === chatGroupId),
        )
      ) {
        return;
      }
      await saveAutomationWorkflows([fallback, ...current]);
    })();
  }, [chatGroupId, existing, fallback]);

  return existing ?? fallback;
}

function StarterAutomationDetails({ starterId }: { starterId: StarterId }) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const starter = useStarterAutomations().find((item) => item.id === starterId);
  const [showPreview, setShowPreview] = useState(false);
  const { values: settingValues } = useStoredSettingValues();
  const removeStarterDraft = useRemoveStarterDraft();

  const saveDraftMutation = useMutation({
    mutationKey: ["automation-draft-template"],
    mutationFn: () => setSettingValue("automation_draft_template", starterId),
    onSuccess: () => sonnerToast.success(t`Automation draft saved`),
    onError: () => sonnerToast.error(t`Could not save the automation draft`),
  });

  const setEnabledMutation = useMutation({
    mutationKey: ["automation-starter-enabled"],
    mutationFn: ({ enabled }: { enabled: boolean }) => {
      const updates: SettingValues = { automation_draft_template: starterId };
      updates[STARTER_AUTOMATIONS[starterId].enabledKey] = enabled;
      return setSettingValues(updates);
    },
    onSuccess: (_, { enabled }) =>
      sonnerToast.success(
        enabled ? t`Automation enabled` : t`Automation disabled`,
      ),
    onError: () => sonnerToast.error(t`Could not update the automation`),
  });

  if (!starter) {
    return null;
  }

  const isEnabled = Boolean(
    settingValues[STARTER_AUTOMATIONS[starterId].enabledKey],
  );
  const targetRaw =
    settingValues[STARTER_AUTOMATIONS[starterId].targetKey] ?? "";
  const isReady =
    starterId === "markdown-export"
      ? targetRaw.trim().length > 0
      : parseAutomationTargetRef(targetRaw) !== null;
  const readinessHint = (() => {
    switch (starterId) {
      case "markdown-export":
        return t`Choose an export folder first.`;
      case "slack-recap":
        return t`Choose a Slack channel first.`;
      case "linear-action-items":
        return t`Choose a Linear team first.`;
      case "notion-project-notes":
        return t`Choose a Notion page first.`;
    }
  })();

  const handleSaveDraft = () => {
    if (!billing.isPro) {
      billing.upgradeToPro();
      return;
    }
    saveDraftMutation.mutate();
  };

  const handleEnable = () => {
    if (!billing.isPro) {
      billing.upgradeToPro();
      return;
    }
    setEnabledMutation.mutate({ enabled: true });
  };

  return (
    <AutomationDetailsLayout
      icon={starter.renderIcon(16)}
      title={starter.title}
      description={starter.description}
      actions={
        <AutomationActionsMenu
          actionLabel={<Trans>Remove automation</Trans>}
          onAction={() => removeStarterDraft.mutate(starterId)}
        />
      }
    >
      <section
        {...stylex.props(styles.draft)}
        aria-labelledby="automation-draft-title"
      >
        <div {...stylex.props(styles.draftHeader)}>
          <div {...stylex.props(styles.minWidth)}>
            <div {...stylex.props(styles.draftTitleRow)}>
              <Lightning
                {...stylex.props(styles.primary)}
                size={17}
                weight="fill"
              />
              <h3
                id="automation-draft-title"
                {...stylex.props(styles.draftTitle)}
              >
                {starter.title}
              </h3>
              {isEnabled ? (
                <Badge variant="outline">
                  <Trans>Enabled</Trans>
                </Badge>
              ) : (
                <Badge variant="outline">
                  <Trans>Draft</Trans>
                </Badge>
              )}
            </div>
            <p {...stylex.props(styles.draftDescription)}>
              <Trans>Steps run from top to bottom.</Trans>
            </p>
          </div>
          <div {...stylex.props(styles.draftActions)}>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => setShowPreview((visible) => !visible)}
            >
              <Eye size={14} />
              {showPreview ? (
                <Trans>Hide preview</Trans>
              ) : (
                <Trans>Preview</Trans>
              )}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled
              title={t`Test runs are not available yet.`}
            >
              <Play size={14} />
              <Trans>Test</Trans>
            </Button>
            <Button
              type="button"
              size="sm"
              onClick={handleSaveDraft}
              disabled={!billing.isReady || saveDraftMutation.isPending}
            >
              <FloppyDisk size={14} />
              {billing.isPro ? (
                <Trans>Save draft</Trans>
              ) : (
                <Trans>Upgrade to save</Trans>
              )}
            </Button>
            {isEnabled ? (
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setEnabledMutation.mutate({ enabled: false })}
                disabled={!billing.isReady || setEnabledMutation.isPending}
              >
                <Trans>Disable</Trans>
              </Button>
            ) : (
              <Button
                type="button"
                size="sm"
                onClick={handleEnable}
                disabled={
                  !billing.isReady ||
                  setEnabledMutation.isPending ||
                  (billing.isPro && !isReady)
                }
                title={billing.isPro && !isReady ? readinessHint : undefined}
              >
                <Lightning size={14} weight="fill" />
                {billing.isPro ? (
                  <Trans>Save &amp; enable</Trans>
                ) : (
                  <Trans>Upgrade to enable</Trans>
                )}
              </Button>
            )}
          </div>
        </div>

        <div {...stylex.props(styles.steps)}>
          {starter.steps.map((step, index) => (
            <div key={`${step.kind}-${step.title}`}>
              <div {...stylex.props(styles.step)}>
                <span
                  {...stylex.props(
                    styles.stepBadge,
                    step.kind === "ai"
                      ? styles.aiBadge
                      : step.kind === "trigger"
                        ? styles.triggerBadge
                        : styles.actionBadge,
                  )}
                >
                  {step.kind === "ai" ? <Sparkle size={13} /> : index + 1}
                </span>
                <span {...stylex.props(styles.stepContent)}>
                  <span {...stylex.props(styles.stepTitleRow)}>
                    <span {...stylex.props(styles.stepTitle)}>
                      {step.title}
                    </span>
                    <Badge variant="outline" size="sm">
                      {step.kind === "ai" ? (
                        <Trans>AI step</Trans>
                      ) : step.kind === "trigger" ? (
                        <Trans>Trigger</Trans>
                      ) : (
                        <Trans>Action</Trans>
                      )}
                    </Badge>
                  </span>
                  <span {...stylex.props(styles.stepDetail)}>
                    {step.detail}
                  </span>
                </span>
              </div>
              {index < starter.steps.length - 1 ? (
                <div {...stylex.props(styles.connector)}>
                  <ArrowRight
                    {...stylex.props(styles.connectorIcon)}
                    size={13}
                  />
                </div>
              ) : null}
            </div>
          ))}
        </div>

        <div {...stylex.props(styles.draftFooter)}>
          {starterId === "markdown-export" ? (
            <MarkdownExportConfig />
          ) : starterId === "slack-recap" ? (
            <SlackRecapConfig />
          ) : starterId === "linear-action-items" ? (
            <LinearIssuesConfig />
          ) : (
            <NotionUpdateConfig />
          )}
          <AutomationLastRunLine
            settingKey={STARTER_AUTOMATIONS[starterId].lastRunKey}
          />
        </div>

        {showPreview ? (
          <div {...stylex.props(styles.preview)}>
            <div {...stylex.props(styles.previewRow)}>
              <Eye {...stylex.props(styles.previewIcon)} size={15} />
              <div>
                <h4 {...stylex.props(styles.previewTitle)}>
                  <Trans>Expected output</Trans>
                </h4>
                <p {...stylex.props(styles.previewDescription)}>
                  {starter.preview}
                </p>
              </div>
            </div>
          </div>
        ) : null}
      </section>
    </AutomationDetailsLayout>
  );
}

const styles = stylex.create({
  actionBadge: {
    backgroundColor: {
      default: "rgb(219 234 254)",
      ":is(.dark *)": "rgb(23 37 84)",
    },
    color: {
      default: "rgb(29 78 216)",
      ":is(.dark *)": "rgb(147 197 253)",
    },
  },
  aiBadge: {
    backgroundColor: {
      default: "rgb(237 233 254)",
      ":is(.dark *)": "rgb(46 16 101)",
    },
    color: {
      default: "rgb(109 40 217)",
      ":is(.dark *)": "rgb(196 181 253)",
    },
  },
  connector: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    height: "1.5rem",
    paddingLeft: "1.5rem",
  },
  connectorIcon: {
    transform: "rotate(90deg)",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
    marginInline: "auto",
    maxWidth: "64rem",
    width: "100%",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
    maxWidth: "42rem",
  },
  detailHeader: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.75rem",
    height: "3rem",
    justifyContent: "space-between",
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
  },
  detailIcon: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    width: "1.75rem",
  },
  detailLayout: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
    minHeight: 0,
  },
  detailScroller: {
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    paddingBottom: "2rem",
    paddingLeft: "1.5rem",
    paddingRight: "1.5rem",
    paddingTop: "0.75rem",
    scrollbarWidth: "none",
    width: "100%",
  },
  detailTitle: {
    fontSize: "0.875rem",
    fontWeight: 600,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  detailTitleRow: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.5rem",
    minWidth: 0,
  },
  draft: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  draftActions: {
    alignItems: "center",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
  },
  draftDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.25rem",
  },
  draftFooter: {
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBlock: "1rem",
    paddingInline: "1.25rem",
  },
  draftHeader: {
    alignItems: "flex-start",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.75rem",
    justifyContent: "space-between",
    paddingBlock: "1rem",
    paddingInline: "1.25rem",
  },
  draftTitle: {
    fontSize: "0.875rem",
    fontWeight: 600,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  draftTitleRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  emptyDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: 1.625,
    marginTop: "0.25rem",
    maxWidth: "24rem",
  },
  emptyIconFrame: {
    alignItems: "center",
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    height: "2.75rem",
    justifyContent: "center",
    width: "2.75rem",
  },
  emptyState: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.muted} 20%, transparent)`,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "dashed",
    borderWidth: "1px",
    display: "flex",
    flexDirection: "column",
    justifyContent: "center",
    minHeight: "14rem",
    paddingBlock: "2.5rem",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  emptyTitle: {
    fontSize: "0.875rem",
    fontWeight: 600,
    marginTop: "1rem",
  },
  headerActions: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  menuTrigger: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  minWidth: {
    minWidth: 0,
  },
  muted: {
    color: colors.mutedForeground,
  },
  page: {
    backgroundColor: {
      default: colors.card,
      ":is(.dark *)": colors.accent,
    },
    display: "flex",
    flex: "1",
    flexDirection: "column",
    overflow: "hidden",
    width: "100%",
  },
  pointer: {
    cursor: "pointer",
  },
  preview: {
    backgroundColor: `color-mix(in srgb, ${colors.muted} 35%, transparent)`,
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBlock: "1rem",
    paddingInline: "1.25rem",
  },
  previewDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: 1.625,
    marginTop: "0.25rem",
  },
  previewIcon: {
    color: colors.mutedForeground,
    marginTop: "0.125rem",
  },
  previewRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.75rem",
  },
  previewTitle: {
    fontSize: "0.75rem",
    fontWeight: 600,
  },
  primary: {
    color: colors.primary,
  },
  scroller: {
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    flex: "1",
    height: "100%",
    overflowY: "auto",
    paddingBottom: "2rem",
    paddingLeft: "1.5rem",
    paddingRight: "1.5rem",
    paddingTop: "0.75rem",
    scrollbarWidth: "none",
    width: "100%",
  },
  step: {
    alignItems: "flex-start",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    padding: "1rem",
  },
  stepBadge: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    fontSize: "0.75rem",
    fontWeight: 600,
    height: "1.75rem",
    justifyContent: "center",
    marginTop: "0.125rem",
    width: "1.75rem",
  },
  stepContent: {
    flex: "1",
    minWidth: 0,
  },
  stepDetail: {
    color: colors.mutedForeground,
    display: "block",
    fontSize: "0.75rem",
    lineHeight: 1.625,
    marginTop: "0.25rem",
  },
  steps: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    padding: "1.25rem",
  },
  stepTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  stepTitleRow: {
    alignItems: "center",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
  },
  titleBlock: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  triggerBadge: {
    backgroundColor: {
      default: "rgb(254 243 199)",
      ":is(.dark *)": "rgb(69 26 3)",
    },
    color: {
      default: "rgb(180 83 9)",
      ":is(.dark *)": "rgb(252 211 77)",
    },
  },
  workflowIcon: {
    color: "rgb(139 92 246)",
  },
});

export { styles as automationSettingsStyles };
