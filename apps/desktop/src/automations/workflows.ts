import type { AutomationRunRecord, AutomationTargetRef } from "./types";

import { setSettingValue, useStoredSettingValue } from "~/settings/queries";
import { id } from "~/shared/utils";

export const WORKFLOW_TRIGGERS = [
  "note_enhanced",
  "meeting_completed",
] as const;
export type WorkflowTrigger = (typeof WORKFLOW_TRIGGERS)[number];

export const WORKFLOW_STEP_TYPES = [
  "slack_recap",
  "notion_update",
  "linear_issues",
  "markdown_export",
] as const;
export type WorkflowStepType = (typeof WORKFLOW_STEP_TYPES)[number];

export type WorkflowStep =
  | {
      id: string;
      type: "slack_recap" | "notion_update" | "linear_issues";
      target: AutomationTargetRef | null;
    }
  | {
      id: string;
      type: "markdown_export";
      directory: string;
    };

export type AutomationWorkflow = {
  id: string;
  title: string;
  enabled: boolean;
  trigger: WorkflowTrigger;
  steps: WorkflowStep[];
  lastRun: AutomationRunRecord | null;
  processedSessionIds: string[];
  chatGroupId: string | null;
};

export function createEmptyWorkflow(
  overrides: Partial<AutomationWorkflow> = {},
): AutomationWorkflow {
  return {
    id: overrides.id ?? id(),
    title: overrides.title ?? "Untitled automation",
    enabled: overrides.enabled ?? false,
    trigger: overrides.trigger ?? "note_enhanced",
    steps: overrides.steps ?? [],
    lastRun: overrides.lastRun ?? null,
    processedSessionIds: overrides.processedSessionIds ?? [],
    chatGroupId: overrides.chatGroupId ?? null,
  };
}

export function createWorkflowStep(type: WorkflowStepType): WorkflowStep {
  if (type === "markdown_export") {
    return { id: id(), type, directory: "" };
  }
  return { id: id(), type, target: null };
}

export function isWorkflowStepReady(step: WorkflowStep): boolean {
  if (step.type === "markdown_export") {
    return step.directory.trim().length > 0;
  }
  return step.target !== null;
}

export function isWorkflowReady(workflow: AutomationWorkflow): boolean {
  return workflow.steps.length > 0 && workflow.steps.every(isWorkflowStepReady);
}

export function parseAutomationWorkflows(
  value: string | undefined,
): AutomationWorkflow[] {
  if (!value) {
    return [];
  }
  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.flatMap((item) => {
      const workflow = parseWorkflow(item);
      return workflow ? [workflow] : [];
    });
  } catch {
    return [];
  }
}

export function serializeAutomationWorkflows(
  workflows: AutomationWorkflow[],
): string {
  return JSON.stringify(workflows);
}

export async function saveAutomationWorkflows(
  workflows: AutomationWorkflow[],
): Promise<void> {
  await setSettingValue(
    "automation_workflows",
    serializeAutomationWorkflows(workflows),
  );
}

export function useAutomationWorkflows(): AutomationWorkflow[] {
  return parseAutomationWorkflows(
    useStoredSettingValue("automation_workflows").value,
  );
}

function parseWorkflow(value: unknown): AutomationWorkflow | null {
  if (!isRecord(value) || typeof value.id !== "string") {
    return null;
  }

  const trigger =
    value.trigger === "meeting_completed"
      ? "meeting_completed"
      : "note_enhanced";
  const steps = Array.isArray(value.steps)
    ? value.steps.flatMap((step) => {
        const parsed = parseStep(step);
        return parsed ? [parsed] : [];
      })
    : [];

  return {
    id: value.id,
    title:
      typeof value.title === "string" ? value.title : "Untitled automation",
    enabled: value.enabled === true,
    trigger,
    steps,
    lastRun: parseLastRun(value.lastRun),
    processedSessionIds: Array.isArray(value.processedSessionIds)
      ? value.processedSessionIds.filter(
          (entry): entry is string => typeof entry === "string",
        )
      : [],
    chatGroupId:
      typeof value.chatGroupId === "string" ? value.chatGroupId : null,
  };
}

function parseStep(value: unknown): WorkflowStep | null {
  if (!isRecord(value) || typeof value.id !== "string") {
    return null;
  }
  if (value.type === "markdown_export") {
    return {
      id: value.id,
      type: "markdown_export",
      directory: typeof value.directory === "string" ? value.directory : "",
    };
  }
  if (
    value.type === "slack_recap" ||
    value.type === "notion_update" ||
    value.type === "linear_issues"
  ) {
    return {
      id: value.id,
      type: value.type,
      target: parseTarget(value.target),
    };
  }
  return null;
}

function parseTarget(value: unknown): AutomationTargetRef | null {
  if (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.name === "string"
  ) {
    return { id: value.id, name: value.name };
  }
  return null;
}

function parseLastRun(value: unknown): AutomationRunRecord | null {
  if (
    isRecord(value) &&
    typeof value.at === "string" &&
    (value.status === "success" || value.status === "error") &&
    typeof value.detail === "string"
  ) {
    return {
      at: value.at,
      status: value.status,
      detail: value.detail,
    };
  }
  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
