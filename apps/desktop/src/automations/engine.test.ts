import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  exportMeetingMarkdown: vi.fn(),
  getStoredSettingValues: vi.fn(),
  setSettingValue: vi.fn(),
  execute: vi.fn(),
  getSession: vi.fn(),
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

vi.mock("@anlg/api-client", () => ({
  listConnections: mocks.listConnections,
  linearCreateIssue: mocks.linearCreateIssue,
  notionAppendUpdate: mocks.exportMeetingMarkdown,
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

describe("custom workflows", () => {
  it("runs an enabled workflow after a summary is ready", async () => {
    storedSettings({
      automation_workflows: JSON.stringify([
        {
          id: "wf-1",
          title: "Recap to Slack",
          enabled: true,
          trigger: "note_enhanced",
          steps: [
            {
              id: "step-1",
              type: "markdown_export",
              directory: "/tmp/exports",
            },
          ],
          lastRun: null,
          processedSessionIds: [],
          chatGroupId: null,
        },
      ]),
    });
    mockDbRows();
    signedInSession();
    mocks.exportMeetingMarkdown.mockResolvedValue({
      status: "ok",
      data: "/tmp/exports/a.md",
    });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).toHaveBeenCalledTimes(1);
    const workflowCalls = mocks.setSettingValue.mock.calls.filter(
      (entry) => entry[0] === "automation_workflows",
    );
    const saved = JSON.parse(
      workflowCalls[workflowCalls.length - 1]?.[1] as string,
    );
    expect(saved[0].lastRun.status).toBe("success");
    expect(saved[0].processedSessionIds).toEqual(["session-1"]);
  });

  it("marks a session processed after a successful step so a later failure does not retry", async () => {
    storedSettings({
      automation_workflows: JSON.stringify([
        {
          id: "wf-1",
          title: "Two markdown exports",
          enabled: true,
          trigger: "note_enhanced",
          steps: [
            {
              id: "step-1",
              type: "markdown_export",
              directory: "/tmp/exports",
            },
            {
              id: "step-2",
              type: "markdown_export",
              directory: "/tmp/exports",
            },
          ],
          lastRun: null,
          processedSessionIds: [],
          chatGroupId: null,
        },
      ]),
    });
    mockDbRows();
    signedInSession();
    mocks.exportMeetingMarkdown
      .mockResolvedValueOnce({ status: "ok", data: "/tmp/exports/note.md" })
      .mockResolvedValueOnce({ status: "error", error: "export unavailable" });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).toHaveBeenCalledTimes(2);
    const firstSave = mocks.setSettingValue.mock.calls.filter(
      (entry) => entry[0] === "automation_workflows",
    );
    const afterFailure = JSON.parse(
      firstSave[firstSave.length - 1]?.[1] as string,
    );
    expect(afterFailure[0].processedSessionIds).toEqual(["session-1"]);
    expect(afterFailure[0].lastRun.status).toBe("error");

    storedSettings({
      automation_workflows: JSON.stringify(afterFailure),
    });
    mocks.exportMeetingMarkdown.mockClear();

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).not.toHaveBeenCalled();
  });

  it("retries a workflow when the first step fails before any side effect", async () => {
    const workflow = {
      id: "wf-1",
      title: "Recap to Slack",
      enabled: true,
      trigger: "note_enhanced",
      steps: [
        {
          id: "step-1",
          type: "markdown_export",
          directory: "/tmp/exports",
        },
      ],
      lastRun: null,
      processedSessionIds: [],
      chatGroupId: null,
    };
    storedSettings({
      automation_workflows: JSON.stringify([workflow]),
    });
    mockDbRows({ recap: [] });
    signedInSession();
    mocks.exportMeetingMarkdown.mockResolvedValueOnce({
      status: "error",
      error: "export unavailable",
    });

    await runNoteEnhancedAutomations("session-1");
    const firstSave = mocks.setSettingValue.mock.calls.filter(
      (entry) => entry[0] === "automation_workflows",
    );
    const afterFailure = JSON.parse(
      firstSave[firstSave.length - 1]?.[1] as string,
    );
    expect(afterFailure[0].processedSessionIds).toEqual([]);
    expect(afterFailure[0].lastRun.status).toBe("error");

    storedSettings({
      automation_workflows: JSON.stringify(afterFailure),
    });
    mockDbRows();
    mocks.exportMeetingMarkdown.mockClear();
    mocks.exportMeetingMarkdown.mockResolvedValue({
      status: "ok",
      data: "/tmp/exports/a.md",
    });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).toHaveBeenCalledTimes(1);
  });

  it("skips disabled or already processed workflows", async () => {
    storedSettings({
      automation_workflows: JSON.stringify([
        {
          id: "wf-1",
          title: "Disabled",
          enabled: false,
          trigger: "note_enhanced",
          steps: [
            {
              id: "step-1",
              type: "markdown_export",
              directory: "/tmp/exports",
            },
          ],
          processedSessionIds: [],
        },
        {
          id: "wf-2",
          title: "Already ran",
          enabled: true,
          trigger: "note_enhanced",
          steps: [
            {
              id: "step-1",
              type: "markdown_export",
              directory: "/tmp/exports",
            },
          ],
          processedSessionIds: ["session-1"],
        },
      ]),
    });

    await runNoteEnhancedAutomations("session-1");

    expect(mocks.exportMeetingMarkdown).not.toHaveBeenCalled();
  });
});
