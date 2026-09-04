import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";

import {
  ArrowRight,
  DotsThree,
  Eye,
  FloppyDisk,
  Lightning,
  Play,
  Sparkle,
} from "@anlg/ui/components/icons";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@anlg/utils";

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
import {
  ResourceShareButton,
  sharedAutomationPayload,
} from "~/resource-sharing";
import { SettingsHydrationBoundary } from "~/settings/hydration-boundary";
import { SettingsPageTitle } from "~/settings/page-title";
import { useNotifyPlanRequired } from "~/settings/plan-gate";
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
        <div className="bg-card dark:bg-accent flex w-full flex-1 flex-col overflow-hidden">
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
    <div className="scroll-fade-y scrollbar-hide h-full w-full flex-1 overflow-y-auto px-6 pt-3 pb-8">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6">
        <div className="flex flex-col gap-2">
          <SettingsPageTitle title={<Trans>Automations</Trans>} />
          <p className="text-muted-foreground max-w-2xl text-sm leading-relaxed">
            <Trans>
              Automate what happens before, during, or after meetings based on
              the conditions you choose.
            </Trans>
          </p>
        </div>

        <section className="border-border bg-muted/20 flex min-h-56 flex-col items-center justify-center rounded-2xl border border-dashed px-6 py-10 text-center">
          <span className="bg-background border-border flex size-11 items-center justify-center rounded-2xl border">
            <Lightning className="text-muted-foreground" size={20} />
          </span>
          <h3 className="mt-4 text-sm font-semibold">
            <Trans>No automation draft yet</Trans>
          </h3>
          <p className="text-muted-foreground mt-1 max-w-sm text-xs leading-relaxed">
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
    <header className="flex h-12 shrink-0 items-center justify-between gap-3 pr-1 pl-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <span className="flex size-7 shrink-0 items-center justify-center">
          {icon}
        </span>
        <h2 className="min-w-0 truncate text-sm font-semibold">{title}</h2>
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
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <AutomationDetailHeader icon={icon} title={title} actions={actions} />
      <div className="scroll-fade-y scrollbar-hide min-h-0 w-full flex-1 overflow-y-auto px-6 pt-3 pb-8">
        <div className="mx-auto flex w-full max-w-5xl flex-col gap-6">
          {description && (
            <p className="text-muted-foreground max-w-2xl text-sm leading-relaxed">
              {description}
            </p>
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
          className="text-muted-foreground hover:text-foreground"
          aria-label={t`Automation actions`}
        >
          <DotsThree className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end">
        <AppFloatingPanel className={appFloatingMenuPanelClassName}>
          <DropdownMenuItem onClick={onAction} className="cursor-pointer">
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
  const notifyPlanRequired = useNotifyPlanRequired();
  const workflows = useAutomationWorkflows();
  const saveWorkflow = useSaveWorkflow();

  const persist = (next: AutomationWorkflow) => {
    saveWorkflow.mutate({ workflows, next });
  };

  const handleEnable = (enabled: boolean) => {
    if (enabled && !billing.isPro) {
      notifyPlanRequired("pro");
      return;
    }
    persist({ ...workflow, enabled });
  };

  return (
    <AutomationDetailsLayout
      icon={<Lightning className="text-violet-500" size={16} weight="fill" />}
      title={title}
      description={description}
      actions={
        <div className="flex items-center gap-2">
          <ResourceShareButton
            resourceType="automation"
            sourceId={workflow.id}
            title={workflow.title.trim() || t`Untitled automation`}
            buildPayload={() => sharedAutomationPayload(workflow)}
          />
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
              <Trans>Save &amp; enable</Trans>
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
  const notifyPlanRequired = useNotifyPlanRequired();
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
      notifyPlanRequired("pro");
      return;
    }
    saveDraftMutation.mutate();
  };

  const handleEnable = () => {
    if (!billing.isPro) {
      notifyPlanRequired("pro");
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
        className="border-border bg-background overflow-hidden rounded-2xl border"
        aria-labelledby="automation-draft-title"
      >
        <div className="border-border flex flex-wrap items-start justify-between gap-3 border-b px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Lightning className="text-primary" size={17} weight="fill" />
              <h3
                id="automation-draft-title"
                className="truncate text-sm font-semibold"
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
            <p className="text-muted-foreground mt-1 text-xs">
              <Trans>Steps run from top to bottom.</Trans>
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
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
              <Trans>Save draft</Trans>
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
                <Trans>Save &amp; enable</Trans>
              </Button>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-2 p-5">
          {starter.steps.map((step, index) => (
            <div key={`${step.kind}-${step.title}`}>
              <div className="border-border bg-card flex items-start gap-3 rounded-xl border p-4">
                <span
                  className={cn([
                    "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full text-xs font-semibold",
                    step.kind === "ai"
                      ? "bg-violet-100 text-violet-700 dark:bg-violet-950 dark:text-violet-300"
                      : step.kind === "trigger"
                        ? "bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300"
                        : "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
                  ])}
                >
                  {step.kind === "ai" ? <Sparkle size={13} /> : index + 1}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium">{step.title}</span>
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
                  <span className="text-muted-foreground mt-1 block text-xs leading-relaxed">
                    {step.detail}
                  </span>
                </span>
              </div>
              {index < starter.steps.length - 1 ? (
                <div className="text-muted-foreground flex h-6 items-center pl-6">
                  <ArrowRight className="rotate-90" size={13} />
                </div>
              ) : null}
            </div>
          ))}
        </div>

        <div className="border-border border-t px-5 py-4">
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
          <div className="border-border bg-muted/35 border-t px-5 py-4">
            <div className="flex items-start gap-3">
              <Eye className="text-muted-foreground mt-0.5" size={15} />
              <div>
                <h4 className="text-xs font-semibold">
                  <Trans>Expected output</Trans>
                </h4>
                <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
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
