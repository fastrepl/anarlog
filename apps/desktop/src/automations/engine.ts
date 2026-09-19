import { commands as localApiCommands } from "@anlg/plugin-local-api";

import {
  type AutomationRunRecord,
  type AutomationTargetRef,
  parseAutomationRunRecord,
  parseAutomationTargetRef,
} from "./types";
import {
  type AutomationWorkflow,
  parseAutomationWorkflows,
  serializeAutomationWorkflows,
  type WorkflowStep,
  type WorkflowTrigger,
} from "./workflows";

import { getStoredSettingValues, setSettingValue } from "~/settings/queries";

export type { AutomationRunRecord, AutomationTargetRef };
export { parseAutomationRunRecord, parseAutomationTargetRef };

const MAX_PROCESSED_SESSIONS = 50;

export async function runMeetingCompletedAutomations(
  sessionId: string,
): Promise<void> {
  try {
    await runMarkdownExport(sessionId);
  } catch (error) {
    console.error("[automations] meeting.completed run failed", error);
  }
  try {
    await runCustomWorkflows(sessionId, "meeting_completed");
  } catch (error) {
    console.error("[automations] meeting.completed workflows failed", error);
  }
}

export async function runNoteEnhancedAutomations(
  sessionId: string,
): Promise<void> {
  try {
    await runCustomWorkflows(sessionId, "note_enhanced");
  } catch (error) {
    console.error("[automations] note.enhanced workflows failed", error);
  }
}

async function runCustomWorkflows(
  sessionId: string,
  trigger: WorkflowTrigger,
): Promise<void> {
  const { values } = await getStoredSettingValues();
  const workflows = parseAutomationWorkflows(values.automation_workflows);
  for (const workflow of workflows) {
    if (!workflow.enabled || workflow.trigger !== trigger) {
      continue;
    }
    if (workflow.steps.length === 0) {
      continue;
    }
    if (workflow.processedSessionIds.includes(sessionId)) {
      continue;
    }
    try {
      await runWorkflow(sessionId, workflow);
    } catch (error) {
      console.error("[automations] workflow run failed", error);
    }
  }
}

async function runWorkflow(
  sessionId: string,
  workflow: AutomationWorkflow,
): Promise<void> {
  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  let markedSessionId: string | undefined;
  const markProcessed = async () => {
    markedSessionId = sessionId;
    await persistWorkflowResult(workflow.id, { sessionId });
  };
  try {
    const details: string[] = [];
    for (const step of workflow.steps) {
      details.push(await executeWorkflowStep(sessionId, step));
      await markProcessed();
    }
    record.detail = details.filter(Boolean).join(" · ") || "ok";
    await persistWorkflowResult(workflow.id, {
      record,
      sessionId,
    });
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] workflow failed", error);
    await persistWorkflowResult(workflow.id, {
      record,
      sessionId: markedSessionId,
    });
  }
}

async function persistWorkflowResult(
  workflowId: string,
  {
    record,
    sessionId,
  }: {
    record?: AutomationRunRecord;
    sessionId?: string;
  },
): Promise<void> {
  const { values } = await getStoredSettingValues();
  const workflows = parseAutomationWorkflows(values.automation_workflows);
  const next = workflows.map((workflow) => {
    if (workflow.id !== workflowId) {
      return workflow;
    }
    return {
      ...workflow,
      lastRun: record ?? workflow.lastRun,
      processedSessionIds: sessionId
        ? appendProcessedSession(workflow.processedSessionIds, sessionId)
        : workflow.processedSessionIds,
    };
  });
  await setSettingValue(
    "automation_workflows",
    serializeAutomationWorkflows(next),
  );
}

function appendProcessedSession(
  processed: string[],
  sessionId: string,
): string[] {
  if (processed.includes(sessionId)) {
    return processed;
  }
  return [...processed, sessionId].slice(-MAX_PROCESSED_SESSIONS);
}

async function executeWorkflowStep(
  sessionId: string,
  step: WorkflowStep,
): Promise<string> {
  if (step.type === "markdown_export") {
    const directory = step.directory.trim();
    if (!directory) {
      throw new Error("choose an export folder first");
    }
    return await executeMarkdownExport(sessionId, directory);
  }
  throw new Error("unsupported automation step");
}

async function runMarkdownExport(sessionId: string): Promise<void> {
  const { values } = await getStoredSettingValues();
  if (!values.automation_markdown_export_enabled) {
    return;
  }
  const directory = (values.automation_markdown_export_directory ?? "").trim();
  if (!directory) {
    return;
  }

  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  try {
    record.detail = await executeMarkdownExport(sessionId, directory);
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] markdown export failed", error);
  }
  await setSettingValue(
    "automation_markdown_export_last_run",
    JSON.stringify(record),
  );
}

async function executeMarkdownExport(
  sessionId: string,
  directory: string,
): Promise<string> {
  const result = await localApiCommands.exportMeetingMarkdown(
    sessionId,
    directory,
  );
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}
