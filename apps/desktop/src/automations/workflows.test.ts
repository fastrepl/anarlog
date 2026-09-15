import { describe, expect, it } from "vitest";

import {
  DEFAULT_MARKDOWN_EXPORT_OPTIONS,
  parseMarkdownExportOptions,
} from "./markdown-export";
import {
  createEmptyWorkflow,
  createWorkflowStep,
  isWorkflowReady,
  parseAutomationWorkflows,
  serializeAutomationWorkflows,
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

describe("Markdown workflow options", () => {
  it("round-trips per-action selections and names independently", () => {
    const options = {
      ...DEFAULT_MARKDOWN_EXPORT_OPTIONS,
      include_memo: false,
      include_transcript: false,
      include_action_items: false,
      filename: "{date} Recap.md",
      include_id_suffix: false,
    };
    const workflow = createEmptyWorkflow({
      steps: [
        {
          id: "summary",
          type: "markdown_export",
          directory: "/exports",
          options,
        },
        {
          id: "all",
          type: "markdown_export",
          directory: "/archive",
          options: DEFAULT_MARKDOWN_EXPORT_OPTIONS,
        },
      ],
    });
    expect(
      parseAutomationWorkflows(serializeAutomationWorkflows([workflow])),
    ).toEqual([workflow]);
  });

  it("defaults malformed fields without losing explicit false values", () => {
    expect(
      parseMarkdownExportOptions({
        include_summary: false,
        include_memo: "false",
        filename: 42,
        include_id_suffix: false,
      }),
    ).toEqual({
      ...DEFAULT_MARKDOWN_EXPORT_OPTIONS,
      include_summary: false,
      include_id_suffix: false,
    });
    expect(parseMarkdownExportOptions(null)).toEqual(
      DEFAULT_MARKDOWN_EXPORT_OPTIONS,
    );
  });

  it("requires a folder and at least one selected element", () => {
    const workflow = createEmptyWorkflow({
      steps: [
        {
          id: "export",
          type: "markdown_export",
          directory: "/exports",
          options: {
            ...DEFAULT_MARKDOWN_EXPORT_OPTIONS,
            include_memo: false,
            include_summary: false,
            include_transcript: false,
            include_action_items: false,
          },
        },
      ],
    });
    expect(isWorkflowReady(workflow)).toBe(false);
    workflow.steps = [
      { id: "legacy", type: "markdown_export", directory: "/exports" },
    ];
    expect(isWorkflowReady(workflow)).toBe(true);
  });
});
