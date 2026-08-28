import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowRight, Plus, Trash } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";

import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import {
  AutomationLastRunLine,
  LinearIssuesConfig,
  MarkdownExportConfig,
  NotionUpdateConfig,
  SlackRecapConfig,
} from "./starter-config";

import {
  createWorkflowStep,
  isWorkflowReady,
  isWorkflowStepReady,
  type AutomationWorkflow,
  type WorkflowStep,
  type WorkflowStepType,
  type WorkflowTrigger,
} from "~/automations/workflows";

export function WorkflowBuilder({
  workflow,
  onChange,
}: {
  workflow: AutomationWorkflow;
  onChange: (workflow: AutomationWorkflow) => void;
}) {
  const { t } = useLingui();

  const update = (patch: Partial<AutomationWorkflow>) => {
    onChange({ ...workflow, ...patch });
  };

  const updateStep = (stepId: string, next: WorkflowStep) => {
    update({
      steps: workflow.steps.map((step) => (step.id === stepId ? next : step)),
    });
  };

  const addStep = (type: WorkflowStepType) => {
    update({ steps: [...workflow.steps, createWorkflowStep(type)] });
  };

  const removeStep = (stepId: string) => {
    update({ steps: workflow.steps.filter((step) => step.id !== stepId) });
  };

  return (
    <section
      className="border-border bg-background overflow-hidden rounded-2xl border"
      aria-labelledby="workflow-builder-title"
    >
      <div className="border-border flex flex-wrap items-start justify-between gap-3 border-b px-5 py-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h3
              id="workflow-builder-title"
              className="truncate text-sm font-semibold"
            >
              <Trans>Workflow</Trans>
            </h3>
            {workflow.enabled ? (
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
            <Trans>Add a trigger, then stack actions like Zapier.</Trans>
          </p>
        </div>
      </div>

      <div className="flex flex-col gap-2 p-5">
        <WorkflowCard
          kind="trigger"
          title={t`When this happens`}
          badge={<Trans>Trigger</Trans>}
        >
          <Select
            value={workflow.trigger}
            onValueChange={(value) =>
              update({ trigger: value as WorkflowTrigger })
            }
          >
            <SelectTrigger className="h-8 w-full max-w-xs text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="note_enhanced">
                <Trans>After the meeting summary is ready</Trans>
              </SelectItem>
              <SelectItem value="meeting_completed">
                <Trans>After the meeting ends</Trans>
              </SelectItem>
            </SelectContent>
          </Select>
        </WorkflowCard>

        {workflow.steps.map((step, index) => (
          <div key={step.id}>
            <div className="text-muted-foreground flex h-6 items-center pl-6">
              <ArrowRight className="rotate-90" size={13} />
            </div>
            <WorkflowCard
              kind="action"
              title={`${t`Then`} ${index + 1}`}
              badge={<Trans>Action</Trans>}
              ready={isWorkflowStepReady(step)}
              onRemove={() => removeStep(step.id)}
            >
              <Select
                value={step.type}
                onValueChange={(value) =>
                  updateStep(
                    step.id,
                    createWorkflowStep(value as WorkflowStepType),
                  )
                }
              >
                <SelectTrigger className="h-8 w-full max-w-xs text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="slack_recap">
                    <Trans>Post a recap to Slack</Trans>
                  </SelectItem>
                  <SelectItem value="notion_update">
                    <Trans>Append an update to Notion</Trans>
                  </SelectItem>
                  <SelectItem value="linear_issues">
                    <Trans>Create Linear issues from action items</Trans>
                  </SelectItem>
                  <SelectItem value="markdown_export">
                    <Trans>Export the meeting as Markdown</Trans>
                  </SelectItem>
                </SelectContent>
              </Select>
              <div className="mt-3">
                <WorkflowStepConfig
                  step={step}
                  onChange={(next) => updateStep(step.id, next)}
                />
              </div>
            </WorkflowCard>
          </div>
        ))}

        <div className="text-muted-foreground flex h-6 items-center pl-6">
          <ArrowRight className="rotate-90" size={13} />
        </div>

        <AddWorkflowStep onAdd={addStep} />
      </div>

      <div className="border-border border-t px-5 py-4">
        {!isWorkflowReady(workflow) ? (
          <p className="text-muted-foreground text-xs">
            <Trans>Add at least one configured action before enabling.</Trans>
          </p>
        ) : null}
        <AutomationLastRunLine lastRun={workflow.lastRun} />
      </div>
    </section>
  );
}

function WorkflowStepConfig({
  step,
  onChange,
}: {
  step: WorkflowStep;
  onChange: (step: WorkflowStep) => void;
}) {
  if (step.type === "markdown_export") {
    return (
      <MarkdownExportConfig
        value={step.directory}
        onChange={(directory) => onChange({ ...step, directory })}
      />
    );
  }
  if (step.type === "slack_recap") {
    return (
      <SlackRecapConfig
        value={step.target}
        onChange={(target) => onChange({ ...step, target })}
      />
    );
  }
  if (step.type === "linear_issues") {
    return (
      <LinearIssuesConfig
        value={step.target}
        onChange={(target) => onChange({ ...step, target })}
      />
    );
  }
  return (
    <NotionUpdateConfig
      value={step.target}
      onChange={(target) => onChange({ ...step, target })}
    />
  );
}

function AddWorkflowStep({
  onAdd,
}: {
  onAdd: (type: WorkflowStepType) => void;
}) {
  const { t } = useLingui();

  return (
    <div className="border-border bg-muted/20 flex items-center justify-between gap-3 rounded-xl border border-dashed p-4">
      <div>
        <p className="text-sm font-medium">
          <Trans>Add an action</Trans>
        </p>
        <p className="text-muted-foreground mt-1 text-xs">
          <Trans>Stack another destination. Steps run top to bottom.</Trans>
        </p>
      </div>
      <Select onValueChange={(value) => onAdd(value as WorkflowStepType)}>
        <SelectTrigger className="h-8 w-44 text-xs">
          <span className="flex items-center gap-1.5">
            <Plus size={12} />
            {t`Add step`}
          </span>
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="slack_recap">
            <Trans>Slack recap</Trans>
          </SelectItem>
          <SelectItem value="notion_update">
            <Trans>Notion update</Trans>
          </SelectItem>
          <SelectItem value="linear_issues">
            <Trans>Linear issues</Trans>
          </SelectItem>
          <SelectItem value="markdown_export">
            <Trans>Markdown export</Trans>
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}

function WorkflowCard({
  kind,
  title,
  badge,
  ready = true,
  onRemove,
  children,
}: {
  kind: "trigger" | "action";
  title: string;
  badge: React.ReactNode;
  ready?: boolean;
  onRemove?: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="border-border bg-card flex items-start gap-3 rounded-xl border p-4">
      <span
        className={cn([
          "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full text-xs font-semibold",
          kind === "trigger"
            ? "bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300"
            : "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
        ])}
      >
        {kind === "trigger" ? "1" : "+"}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">{title}</span>
          <Badge variant="outline" size="sm">
            {badge}
          </Badge>
          {!ready ? (
            <Badge variant="outline" size="sm">
              <Trans>Needs setup</Trans>
            </Badge>
          ) : null}
          {onRemove ? (
            <Button
              type="button"
              size="icon"
              variant="ghost"
              className="text-muted-foreground ml-auto size-7"
              onClick={onRemove}
              aria-label="Remove step"
            >
              <Trash size={13} />
            </Button>
          ) : null}
        </div>
        <div className="mt-3">{children}</div>
      </div>
    </div>
  );
}

export function useSaveWorkflow() {
  const { t } = useLingui();
  return useMutation({
    mutationKey: ["automation-workflow-save"],
    mutationFn: async ({
      workflows,
      next,
    }: {
      workflows: AutomationWorkflow[];
      next: AutomationWorkflow;
    }) => {
      const { saveAutomationWorkflows } =
        await import("~/automations/workflows");
      await saveAutomationWorkflows(
        workflows.some((workflow) => workflow.id === next.id)
          ? workflows.map((workflow) =>
              workflow.id === next.id ? next : workflow,
            )
          : [next, ...workflows],
      );
    },
    onError: () => sonnerToast.error(t`Could not update the automation`),
  });
}
