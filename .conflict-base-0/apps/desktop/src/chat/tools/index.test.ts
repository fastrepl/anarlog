import { afterEach, describe, expect, it, vi } from "vitest";

import { getMeeting } from "@hypr/plugin-db";

import { buildChatTools, type ToolDependencies } from "./index";

function dependencies(): ToolDependencies {
  return {
    search: vi.fn(),
    getContactSearchResults: vi.fn(),
    getCalendarEventSearchResults: vi.fn(),
    getSessionId: vi.fn(),
    getEnhancedNoteId: vi.fn(),
    openEditTab: vi.fn(),
    getAuthHeaders: vi.fn(),
  };
}

describe("chat tool registration", () => {
  afterEach(() => {
    vi.mocked(getMeeting).mockReset();
    vi.restoreAllMocks();
  });

  it("registers meeting vocabulary without legacy shell-shaped names", () => {
    const tools = buildChatTools(dependencies());

    expect(tools).toHaveProperty("list_meetings");
    expect(tools).toHaveProperty("get_meeting");
    expect(tools).toHaveProperty("get_meeting_transcript");
    expect(tools).toHaveProperty("get_recurring_meeting_history");
    expect(tools).toHaveProperty("search_meetings");
    expect(tools).toHaveProperty("search_meeting_content");
    expect(tools).toHaveProperty("find_related_meetings");
    expect(tools).not.toHaveProperty("search_sessions");
    expect(tools).not.toHaveProperty("grep_notes");
    expect(tools).not.toHaveProperty("read_note");
    expect(tools).not.toHaveProperty("read_current_note");
    expect(tools).not.toHaveProperty("list_related_notes");
  });

  it("does not log meeting tool inputs or outputs", async () => {
    vi.mocked(getMeeting).mockResolvedValue({
      title: "private output",
    } as never);
    const log = vi.spyOn(console, "log").mockImplementation(() => {});
    const tools = buildChatTools(dependencies());

    await (tools.get_meeting as any).execute({
      meeting_id: "private input",
    });

    expect(JSON.stringify(log.mock.calls)).not.toContain("private");
  });

  it("does not log meeting tool errors", async () => {
    vi.mocked(getMeeting).mockRejectedValue(new Error("private error"));
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const tools = buildChatTools(dependencies());

    await expect(
      (tools.get_meeting as any).execute({ meeting_id: "private input" }),
    ).rejects.toThrow("private error");

    expect(JSON.stringify(error.mock.calls)).not.toContain("private");
  });
});
