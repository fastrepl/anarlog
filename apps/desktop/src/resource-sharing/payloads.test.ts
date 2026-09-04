import { describe, expect, it } from "vitest";

import {
  parseSharedAutomationPayload,
  parseSharedFolderPayload,
  parseSharedTemplatePayload,
  sharedAutomationPayload,
  sharedTemplatePayload,
} from "./payloads";

import type { AutomationWorkflow } from "~/automations/workflows";
import { DEFAULT_TEMPLATE_ICON } from "~/templates/template-icon";

describe("shared resource payloads", () => {
  it("removes private destinations and runtime state from automation recipes", () => {
    const workflow: AutomationWorkflow = {
      id: "weekly-recap",
      title: "Weekly recap",
      enabled: true,
      trigger: "meeting_completed",
      steps: [
        {
          id: "slack",
          type: "slack_recap",
          target: { id: "C123", name: "Leadership" },
        },
        {
          id: "markdown",
          type: "markdown_export",
          directory: "/Users/me/Exports",
        },
      ],
      lastRun: {
        at: "2026-09-04T00:00:00.000Z",
        status: "success",
        detail: "Sent",
      },
      processedSessionIds: ["private-session"],
      chatGroupId: "private-chat",
    };

    const payload = sharedAutomationPayload(workflow);
    const parsed = parseSharedAutomationPayload(payload);

    expect(parsed).toMatchObject({
      id: "weekly-recap",
      enabled: false,
      lastRun: null,
      processedSessionIds: [],
      chatGroupId: null,
      steps: [
        { id: "slack", type: "slack_recap", target: null },
        { id: "markdown", type: "markdown_export", directory: "" },
      ],
    });
  });

  it("round-trips a shared template draft", () => {
    const parsed = parseSharedTemplatePayload(
      sharedTemplatePayload({
        id: "standup",
        title: "Standup",
        description: "Daily team update",
        pinned: true,
        icon: DEFAULT_TEMPLATE_ICON,
        targets: ["engineering"],
        sections: [{ title: "Blockers", description: "What is blocked?" }],
      }),
    );

    expect(parsed).toEqual({
      title: "Standup",
      description: "Daily team update",
      category: undefined,
      icon: DEFAULT_TEMPLATE_ICON,
      targets: ["engineering"],
      sections: [{ title: "Blockers", description: "What is blocked?" }],
    });
  });

  it("accepts nested folder copies but rejects path traversal", () => {
    expect(
      parseSharedFolderPayload({
        version: 1,
        path: "Customers",
        instructions: "Keep decisions concise",
        notes: [
          {
            title: "Acme kickoff",
            relativeFolderPath: "Acme/Calls",
            body: { type: "doc", content: [] },
          },
        ],
      }),
    ).toMatchObject({
      path: "Customers",
      notes: [{ relativeFolderPath: "Acme/Calls" }],
    });

    expect(() =>
      parseSharedFolderPayload({
        version: 1,
        path: "Customers",
        instructions: "",
        notes: [
          {
            title: "Escape",
            relativeFolderPath: "../Private",
            body: { type: "doc", content: [] },
          },
        ],
      }),
    ).toThrow("This shared folder is invalid");
  });
});
