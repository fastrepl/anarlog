import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn(),
  createSessionForEvent: vi.fn(),
  ensureFolderCatalog: vi.fn(() => Promise.resolve("Work")),
  eventFireAndForget: vi.fn(() => Promise.resolve()),
  execute: vi.fn(),
  updateSession: vi.fn(() => Promise.resolve()),
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: { eventFireAndForget: mocks.eventFireAndForget },
}));

vi.mock("@anlg/plugin-session", () => ({
  commands: {
    createSession: mocks.createSession,
    createSessionForEvent: mocks.createSessionForEvent,
  },
}));

vi.mock("./sessions", () => ({
  updateSession: mocks.updateSession,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/session/folder-catalog", () => ({
  ensureFolderCatalog: mocks.ensureFolderCatalog,
}));

import { getOrCreateSessionForEventId } from "./creation";

function mockEventDatabase({
  seriesId = "series-1",
  ruleFolder = "Work",
  folderExists = true,
}: {
  seriesId?: string | null;
  ruleFolder?: string | null;
  folderExists?: boolean;
} = {}) {
  mocks.execute.mockImplementation(async (sql: string) => {
    if (sql.includes("FROM events")) {
      return [
        {
          id: "event-1",
          participants_json: null,
          recurrence_series_id: seriesId,
        },
      ];
    }
    if (sql.includes("FROM app_settings")) {
      return [
        {
          value_json: JSON.stringify(
            ruleFolder
              ? [{ series_id: "series-1", folder_path: ruleFolder }]
              : [],
          ),
        },
      ];
    }
    if (sql.includes("FROM folders")) {
      return folderExists ? [{ present: 1 }] : [];
    }
    return [];
  });
}

describe("getOrCreateSessionForEventId series folder rules", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.createSessionForEvent.mockResolvedValue({
      status: "ok",
      data: { created: true, session_id: "session-1" },
    });
  });

  it("files a newly created event note into the series folder", async () => {
    mockEventDatabase();

    const sessionId = await getOrCreateSessionForEventId("event-1", "Standup");

    expect(sessionId).toBe("session-1");
    expect(mocks.updateSession).toHaveBeenCalledWith("session-1", {
      folder_id: "Work",
    });
  });

  it("skips the rule when the folder no longer exists", async () => {
    mockEventDatabase({ folderExists: false });

    await getOrCreateSessionForEventId("event-1", "Standup");

    expect(mocks.updateSession).not.toHaveBeenCalled();
  });

  it("leaves the note unfiled when the series has no rule", async () => {
    mockEventDatabase({ ruleFolder: null });

    await getOrCreateSessionForEventId("event-1", "Standup");

    expect(mocks.updateSession).not.toHaveBeenCalled();
  });

  it("does not re-apply the rule when opening an existing event note", async () => {
    mockEventDatabase();
    mocks.createSessionForEvent.mockResolvedValue({
      status: "ok",
      data: { created: false, session_id: "session-1" },
    });

    await getOrCreateSessionForEventId("event-1", "Standup");

    expect(mocks.updateSession).not.toHaveBeenCalled();
  });
});
