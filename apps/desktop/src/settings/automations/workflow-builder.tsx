import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowRight, Plus, Trash } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
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
      {...stylex.props(styles.root)}
      aria-labelledby="workflow-builder-title"
    >
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.minWidth)}>
          <div {...stylex.props(styles.titleRow)}>
            <h3 id="workflow-builder-title" {...stylex.props(styles.title)}>
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
          <p {...stylex.props(styles.description)}>
            <Trans>Add a trigger, then stack actions like Zapier.</Trans>
          </p>
        </div>
      </div>

      <div {...stylex.props(styles.steps)}>
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
            <SelectTrigger sx={styles.stepSelect}>
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
            <div {...stylex.props(styles.connector)}>
              <ArrowRight {...stylex.props(styles.connectorIcon)} size={13} />
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
                <SelectTrigger sx={styles.stepSelect}>
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
              <div {...stylex.props(styles.config)}>
                <WorkflowStepConfig
                  step={step}
                  onChange={(next) => updateStep(step.id, next)}
                />
              </div>
            </WorkflowCard>
          </div>
        ))}

        <div {...stylex.props(styles.connector)}>
          <ArrowRight {...stylex.props(styles.connectorIcon)} size={13} />
        </div>

        <AddWorkflowStep onAdd={addStep} />
      </div>

      <div {...stylex.props(styles.footer)}>
        {!isWorkflowReady(workflow) ? (
          <p {...stylex.props(styles.mutedText)}>
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
    <div {...stylex.props(styles.addStep)}>
      <div>
        <p {...stylex.props(styles.cardTitle)}>
          <Trans>Add an action</Trans>
        </p>
        <p {...stylex.props(styles.description)}>
          <Trans>Stack another destination. Steps run top to bottom.</Trans>
        </p>
      </div>
      <Select onValueChange={(value) => onAdd(value as WorkflowStepType)}>
        <SelectTrigger sx={styles.addStepSelect}>
          <span {...stylex.props(styles.addStepLabel)}>
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
    <div {...stylex.props(styles.card)}>
      <span
        {...stylex.props(
          styles.stepBadge,
          kind === "trigger" ? styles.triggerBadge : styles.actionBadge,
        )}
      >
        {kind === "trigger" ? "1" : "+"}
      </span>
      <div {...stylex.props(styles.cardContent)}>
        <div {...stylex.props(styles.cardHeader)}>
          <span {...stylex.props(styles.cardTitle)}>{title}</span>
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
              sx={styles.removeButton}
              onClick={onRemove}
              aria-label="Remove step"
            >
              <Trash size={13} />
            </Button>
          ) : null}
        </div>
        <div {...stylex.props(styles.config)}>{children}</div>
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
  addStep: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.muted} 20%, transparent)`,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "dashed",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    padding: "1rem",
  },
  addStepLabel: {
    alignItems: "center",
    display: "flex",
    gap: "0.375rem",
  },
  addStepSelect: {
    fontSize: "0.75rem",
    height: "2rem",
    width: "11rem",
  },
  card: {
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
  cardContent: {
    flex: "1",
    minWidth: 0,
  },
  cardHeader: {
    alignItems: "center",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
  },
  cardTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  config: {
    marginTop: "0.75rem",
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
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.25rem",
  },
  footer: {
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBottom: "1rem",
    paddingLeft: "1.25rem",
    paddingRight: "1.25rem",
    paddingTop: "1rem",
  },
  header: {
    alignItems: "flex-start",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.75rem",
    justifyContent: "space-between",
    paddingBottom: "1rem",
    paddingLeft: "1.25rem",
    paddingRight: "1.25rem",
    paddingTop: "1rem",
  },
  minWidth: {
    minWidth: 0,
  },
  mutedText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  removeButton: {
    color: colors.mutedForeground,
    height: "1.75rem",
    marginLeft: "auto",
    width: "1.75rem",
  },
  root: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
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
  steps: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    padding: "1.25rem",
  },
  stepSelect: {
    fontSize: "0.75rem",
    height: "2rem",
    maxWidth: "20rem",
    width: "100%",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 600,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  titleRow: {
    alignItems: "center",
    display: "flex",
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
});
