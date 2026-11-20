import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNamedFolder: vi.fn(),
  createSession: vi.fn(),
  execute: vi.fn(),
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: () => ({ data: [] }),
}));

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
  updateFolderInstructions: vi.fn(),
}));

vi.mock("~/session/queries/creation", () => ({
  createSession: mocks.createSession,
}));

import {
  parseSharedAutomationPayload,
  parseSharedFolderPayload,
  parseSharedTemplatePayload,
  sharedAutomationPayload,
  importSharedFolder,
  sharedFolderPayload,
  sharedTemplatePayload,
} from "./payloads";

import {
  createEmptyWorkflow,
  type AutomationWorkflow,
} from "~/automations/workflows";
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

  it("shares only notes literally inside the shared folder", async () => {
    mocks.execute.mockImplementation(async (sql: string) =>
      sql.includes("FROM folders")
        ? [{ instructions: "" }]
        : [
            {
              title: "Inside",
              folder_path: "Q1_2026/Notes",
              body: "",
              body_format: "markdown",
            },
            {
              title: "Look-alike",
              folder_path: "Q1-2026/Notes",
              body: "",
              body_format: "markdown",
            },
          ],
    );

    const payload = await sharedFolderPayload("Q1_2026");

    expect(
      (payload.notes as { title: string }[]).map((note) => note.title),
    ).toEqual(["Inside"]);
  });

  it("keeps a renamed note's subfolder even when its full path is longer than new paths may be", async () => {
    const relative = ["a", "b", "c"].map((c) => c.repeat(64)).join("/");
    mocks.execute.mockImplementation(async (sql: string) =>
      sql.includes("FROM folders")
        ? [{ instructions: "" }]
        : [
            {
              title: "Deep",
              folder_path: `Archived/${relative}`,
              body: "",
              body_format: "markdown",
            },
          ],
    );

    const payload = await sharedFolderPayload("Archived");

    expect(
      (payload.notes as { relativeFolderPath: string }[])[0]
        ?.relativeFolderPath,
    ).toBe(relative);
  });

  it("refuses to import a copy whose nested folder no longer fits, before creating anything", async () => {
    mocks.execute.mockResolvedValue([]);
    const relative = ["a", "b", "c"].map((c) => c.repeat(64)).join("/");

    await expect(
      importSharedFolder(
        {
          version: 1,
          path: "Archived",
          instructions: "",
          notes: [{ title: "Deep", relativeFolderPath: relative, body: {} }],
        },
        "user-1",
      ),
    ).rejects.toThrow("nested too deeply");
    expect(mocks.createNamedFolder).not.toHaveBeenCalled();
    expect(mocks.createSession).not.toHaveBeenCalled();
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

it("removes Google Drive destinations and delivery history from shared automations", () => {
  const workflow = createEmptyWorkflow({
    steps: [
      {
        id: "drive",
        type: "google_drive_export",
        connectionId: "private-connection",
        target: { id: "private-folder", name: "Client" },
      },
    ],
    driveExports: [
      {
        sessionId: "private-meeting",
        stepId: "drive",
        connectionId: "private-connection",
        folderId: "private-folder",
        fileId: "private-file",
        status: "error",
        detail: "private-error",
        at: "2026-09-18",
      },
    ],
  });
  const serialized = JSON.stringify(sharedAutomationPayload(workflow));
  expect(serialized).not.toContain("private-");
  expect(serialized).not.toContain("Client");
});
