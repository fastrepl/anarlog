import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  exportMeetingMarkdown: vi.fn(),
  getStoredSettingValues: vi.fn(),
  setSettingValue: vi.fn(),
  execute: vi.fn(),
  getSession: vi.fn(),
  sendSlackRecap: vi.fn(),
  listConnections: vi.fn(),
  linearCreateIssue: vi.fn(),
  notionAppendUpdate: vi.fn(),
}));

vi.mock("@anlg/plugin-local-api", () => ({
  commands: { exportMeetingMarkdown: mocks.exportMeetingMarkdown },
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: mocks.getStoredSettingValues,
  setSettingValue: mocks.setSettingValue,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/auth/client", () => ({
  supabase: { auth: { getSession: mocks.getSession } },
}));

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.test" },
}));

vi.mock("~/session-sharing/delivery-client", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  sendSlackRecap: mocks.sendSlackRecap,
}));

vi.mock("~/session-sharing/invitation-management", () => ({
  getSessionShareSenderName: () => "Test User",
}));

vi.mock("@anlg/api-client", () => ({
  listConnections: mocks.listConnections,
  linearCreateIssue: mocks.linearCreateIssue,
  notionAppendUpdate: mocks.notionAppendUpdate,
}));

vi.mock("@anlg/api-client/client", () => ({
  createClient: () => ({}),
}));

vi.mock("@anlg/editor/markdown", () => ({
  json2md: (json: { text?: string }) => json.text ?? "",
}));

import {
  parseAutomationRunRecord,
  parseAutomationTargetRef,
  runMeetingCompletedAutomations,
  runNoteEnhancedAutomations,
} from "./engine";

function storedSettings(values: Record<string, unknown>) {
  mocks.getStoredSettingValues.mockResolvedValue({
    values,
    hasValues: new Set(Object.keys(values)),
  });
}

function recordedRun(settingKey: string) {
  const calls = mocks.setSettingValue.mock.calls.filter(
    (entry) => entry[0] === settingKey,
  );
  const call = calls[calls.length - 1];
  return call ? parseAutomationRunRecord(call[1] as string) : null;
}

function signedInSession() {
  mocks.getSession.mockResolvedValue({
    data: {
      session: {
        access_token: "token-1",
        user: { is_anonymous: false, email: "user@example.com" },
      },
    },
    error: null,
  });
}

const RECAP_ROW = {
  session_title: "Weekly Sync",
  occurred_at: "2026-08-07T10:00:00Z",
  body: JSON.stringify({ text: "Decisions were made." }),
  body_format: "prosemirror_json",
};

function mockDbRows({
  recap = [RECAP_ROW],
  actionItems = [],
  summaryDoc = [],
}: {
  recap?: unknown[];
  actionItems?: unknown[];
  summaryDoc?: unknown[];
} = {}) {
  mocks.execute.mockImplementation((sql: string) => {
    if (sql.includes("FROM action_items")) {
      return Promise.resolve(actionItems);
    }
    if (sql.includes("FROM sessions s")) {
      return Promise.resolve(recap);
    }
    return Promise.resolve(summaryDoc);
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.setSettingValue.mockResolvedValue(undefined);
  mocks.listConnections.mockResolvedValue({
    data: {
      connections: [
        { connection_id: "conn-linear", integration_id: "linear" },
        { connection_id: "conn-notion", integration_id: "notion" },
      ],
    },
    error: undefined,
  });
});

describe("runMeetingCompletedAutomations (markdown export)", () => {
  it("does nothing while the automation is disabled", async () => {
    storedSettings({
      automation_markdown_export_enabled: false,
      automation_markdown_export_directory: "/exports",
    });

    await runMeetingCompletedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).not.toHaveBeenCalled();
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("exports the meeting and records a successful run", async () => {
    storedSettings({
      automation_markdown_export_enabled: true,
      automation_markdown_export_directory: "/exports",
    });
    mocks.exportMeetingMarkdown.mockResolvedValue({
      status: "ok",
      data: "/exports/2026-08-07 Standup [abc123].md",
    });

    await runMeetingCompletedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).toHaveBeenCalledWith(
      "session-1",
      "/exports",
    );
    expect(recordedRun("automation_markdown_export_last_run")).toMatchObject({
      status: "success",
      detail: "/exports/2026-08-07 Standup [abc123].md",
    });
  });

  it("records a failed run when the export command errors", async () => {
    storedSettings({
      automation_markdown_export_enabled: true,
      automation_markdown_export_directory: "/exports",
    });
    mocks.exportMeetingMarkdown.mockResolvedValue({
      status: "error",
      error: "could not write markdown export: denied",
    });

    await runMeetingCompletedAutomations("session-1");

    expect(recordedRun("automation_markdown_export_last_run")).toMatchObject({
      status: "error",
      detail: "could not write markdown export: denied",
    });
  });
});

describe("runNoteEnhancedAutomations (slack recap)", () => {
  it("posts the summary to the configured channel", async () => {
    storedSettings({
      automation_slack_recap_enabled: true,
      automation_slack_recap_channel: JSON.stringify({
        id: "C123",
        name: "general",
      }),
    });
    mockDbRows();
    signedInSession();
    mocks.sendSlackRecap.mockResolvedValue(undefined);

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.sendSlackRecap).toHaveBeenCalledWith(
      expect.objectContaining({
        apiBaseUrl: "https://api.test",
        accessToken: "token-1",
        channel: "C123",
        text: expect.stringContaining("Decisions were made."),
      }),
    );
    expect(recordedRun("automation_slack_recap_last_run")).toMatchObject({
      status: "success",
      detail: "#general",
    });
  });

  it("records an error when no summary exists yet", async () => {
    storedSettings({
      automation_slack_recap_enabled: true,
      automation_slack_recap_channel: JSON.stringify({
        id: "C123",
        name: "general",
      }),
    });
    mockDbRows({ recap: [] });
    signedInSession();

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.sendSlackRecap).not.toHaveBeenCalled();
    expect(recordedRun("automation_slack_recap_last_run")).toMatchObject({
      status: "error",
    });
  });

  it("skips when the channel is not configured", async () => {
    storedSettings({ automation_slack_recap_enabled: true });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.sendSlackRecap).not.toHaveBeenCalled();
    expect(recordedRun("automation_slack_recap_last_run")).toBeNull();
  });

  it("posts once per session and records the processed session", async () => {
    storedSettings({
      automation_slack_recap_enabled: true,
      automation_slack_recap_channel: JSON.stringify({
        id: "C123",
        name: "general",
      }),
      automation_slack_recap_processed: JSON.stringify(["session-1"]),
    });
    mockDbRows();
    signedInSession();

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.sendSlackRecap).not.toHaveBeenCalled();
    expect(recordedRun("automation_slack_recap_last_run")).toBeNull();
  });
});

describe("runNoteEnhancedAutomations (linear issues)", () => {
  const linearSettings = {
    automation_linear_issues_enabled: true,
    automation_linear_issues_team: JSON.stringify({
      id: "team-1",
      name: "Core",
    }),
  };

  it("creates one issue per open action item", async () => {
    storedSettings(linearSettings);
    mockDbRows({
      actionItems: [{ text: "Ship the fix" }, { text: "Email the customer" }],
    });
    signedInSession();
    mocks.linearCreateIssue.mockResolvedValue({ data: {}, error: undefined });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.linearCreateIssue).toHaveBeenCalledTimes(2);
    expect(mocks.linearCreateIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        body: expect.objectContaining({
          connection_id: "conn-linear",
          team_id: "team-1",
          title: "Ship the fix",
        }),
      }),
    );
    expect(recordedRun("automation_linear_issues_last_run")).toMatchObject({
      status: "success",
      detail: "2 issues in Core",
    });
    const processedCall = mocks.setSettingValue.mock.calls.find(
      (entry) => entry[0] === "automation_linear_issues_processed",
    );
    expect(JSON.parse(processedCall?.[1] as string)).toEqual(["session-1"]);
  });

  it("skips sessions that were already processed", async () => {
    storedSettings({
      ...linearSettings,
      automation_linear_issues_processed: JSON.stringify(["session-1"]),
    });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.linearCreateIssue).not.toHaveBeenCalled();
    expect(recordedRun("automation_linear_issues_last_run")).toBeNull();
  });

  it("falls back to unchecked task items in the summary document", async () => {
    storedSettings(linearSettings);
    mockDbRows({
      actionItems: [],
      summaryDoc: [
        {
          body: JSON.stringify({
            type: "doc",
            content: [
              {
                type: "taskList",
                content: [
                  {
                    type: "taskItem",
                    attrs: { checked: false },
                    content: [{ type: "text", text: "Review the deck" }],
                  },
                  {
                    type: "taskItem",
                    attrs: { checked: true },
                    content: [{ type: "text", text: "Done already" }],
                  },
                ],
              },
            ],
          }),
          body_format: "prosemirror_json",
        },
      ],
    });
    signedInSession();
    mocks.linearCreateIssue.mockResolvedValue({ data: {}, error: undefined });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.linearCreateIssue).toHaveBeenCalledTimes(1);
    expect(mocks.linearCreateIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        body: expect.objectContaining({ title: "Review the deck" }),
      }),
    );
  });

  it("records a success without issues when nothing is actionable", async () => {
    storedSettings(linearSettings);
    mockDbRows({ actionItems: [], summaryDoc: [] });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.linearCreateIssue).not.toHaveBeenCalled();
    expect(recordedRun("automation_linear_issues_last_run")).toMatchObject({
      status: "success",
      detail: "no action items found for this meeting",
    });
  });

  it("marks the session processed before creating, so a partial failure never duplicates", async () => {
    storedSettings(linearSettings);
    mockDbRows({
      actionItems: [{ text: "First item" }, { text: "Second item" }],
    });
    signedInSession();
    mocks.linearCreateIssue
      .mockResolvedValueOnce({ data: {}, error: undefined })
      .mockResolvedValueOnce({
        data: undefined,
        error: { error: { message: "rate limited" } },
      });

    await runNoteEnhancedAutomations("session-1");

    const processedCall = mocks.setSettingValue.mock.calls.find(
      (entry) => entry[0] === "automation_linear_issues_processed",
    );
    expect(JSON.parse(processedCall?.[1] as string)).toEqual(["session-1"]);
    expect(recordedRun("automation_linear_issues_last_run")).toMatchObject({
      status: "error",
      detail: "rate limited",
    });
  });
});

describe("runNoteEnhancedAutomations (notion update)", () => {
  it("appends a dated update to the configured page", async () => {
    storedSettings({
      automation_notion_update_enabled: true,
      automation_notion_update_page: JSON.stringify({
        id: "page-1",
        name: "Project Apollo",
      }),
    });
    mockDbRows();
    signedInSession();
    mocks.notionAppendUpdate.mockResolvedValue({ data: {}, error: undefined });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.notionAppendUpdate).toHaveBeenCalledWith(
      expect.objectContaining({
        body: {
          connection_id: "conn-notion",
          page_id: "page-1",
          heading: "2026-08-07 — Weekly Sync",
          markdown: "Decisions were made.",
        },
      }),
    );
    expect(recordedRun("automation_notion_update_last_run")).toMatchObject({
      status: "success",
      detail: "Project Apollo",
    });
  });

  it("records an error when the connection is missing", async () => {
    storedSettings({
      automation_notion_update_enabled: true,
      automation_notion_update_page: JSON.stringify({
        id: "page-1",
        name: "Project Apollo",
      }),
    });
    mockDbRows();
    signedInSession();
    mocks.listConnections.mockResolvedValue({
      data: { connections: [] },
      error: undefined,
    });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.notionAppendUpdate).not.toHaveBeenCalled();
    expect(recordedRun("automation_notion_update_last_run")).toMatchObject({
      status: "error",
      detail: "connect notion to run this automation",
    });
  });

  it("appends once per session", async () => {
    storedSettings({
      automation_notion_update_enabled: true,
      automation_notion_update_page: JSON.stringify({
        id: "page-1",
        name: "Project Apollo",
      }),
      automation_notion_update_processed: JSON.stringify(["session-1"]),
    });
    mockDbRows();
    signedInSession();

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.notionAppendUpdate).not.toHaveBeenCalled();
    expect(recordedRun("automation_notion_update_last_run")).toBeNull();
  });
});

describe("parsers", () => {
  it("round-trips run records and rejects malformed values", () => {
    const record = {
      at: "2026-08-07T12:00:00.000Z",
      status: "success",
      detail: "/exports/file.md",
    };
    expect(parseAutomationRunRecord(JSON.stringify(record))).toEqual(record);
    expect(parseAutomationRunRecord(undefined)).toBeNull();
    expect(parseAutomationRunRecord("{broken")).toBeNull();
    expect(parseAutomationRunRecord('{"status":"success"}')).toBeNull();
  });

  it("parses target refs and rejects malformed values", () => {
    expect(parseAutomationTargetRef('{"id":"C1","name":"general"}')).toEqual({
      id: "C1",
      name: "general",
    });
    expect(parseAutomationTargetRef(undefined)).toBeNull();
    expect(parseAutomationTargetRef('{"id":"C1"}')).toBeNull();
    expect(parseAutomationTargetRef("{broken")).toBeNull();
  });
});
