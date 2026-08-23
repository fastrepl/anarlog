import { describe, expect, it } from "vitest";

import {
  createEmptyWorkflow,
  createWorkflowStep,
  isWorkflowReady,
  parseAutomationWorkflows,
} from "./workflows";

describe("automation workflows", () => {
  it("parses persisted workflows and drops malformed entries", () => {
    const workflows = parseAutomationWorkflows(
      JSON.stringify([
        {
          id: "wf-1",
          title: "Recap",
          enabled: true,
          trigger: "meeting_completed",
          steps: [
            {
              id: "step-1",
              type: "markdown_export",
              directory: "/exports",
            },
            { id: "bad" },
          ],
          lastRun: {
            at: "2026-08-07T12:00:00.000Z",
            status: "success",
            detail: "ok",
          },
          processedSessionIds: ["session-1", 2],
          chatGroupId: "group-1",
        },
        { title: "missing id" },
      ]),
    );

    expect(workflows).toEqual([
      {
        id: "wf-1",
        title: "Recap",
        enabled: true,
        trigger: "meeting_completed",
        steps: [
          {
            id: "step-1",
            type: "markdown_export",
            directory: "/exports",
          },
        ],
        lastRun: {
          at: "2026-08-07T12:00:00.000Z",
          status: "success",
          detail: "ok",
        },
        processedSessionIds: ["session-1"],
        chatGroupId: "group-1",
      },
    ]);
  });

  it("treats a workflow as ready only when every action is configured", () => {
    const workflow = createEmptyWorkflow({
      steps: [createWorkflowStep("slack_recap")],
    });

    expect(isWorkflowReady(workflow)).toBe(false);

    workflow.steps = [
      {
        id: "step-1",
        type: "slack_recap",
        target: { id: "C1", name: "general" },
      },
    ];
    expect(isWorkflowReady(workflow)).toBe(true);
  });
});
