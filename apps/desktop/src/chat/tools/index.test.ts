import { afterEach, describe, expect, it, vi } from "vitest";

import { getMeeting, getRecurringMeetingHistory } from "@anlg/plugin-db";

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

const access = { canUsePro: () => true, onProRequired: vi.fn() };

describe("chat tool registration", () => {
  afterEach(() => {
    vi.mocked(getMeeting).mockReset();
    vi.restoreAllMocks();
  });

  it("registers meeting vocabulary without legacy shell-shaped names", () => {
    const tools = buildChatTools(dependencies(), access);

    expect(tools).toHaveProperty("list_meetings");
    expect(tools).toHaveProperty("get_meeting");
    expect(tools).toHaveProperty("get_meeting_transcript");
    expect(tools).toHaveProperty("get_recurring_meeting_history");
    expect(tools).toHaveProperty("search_meetings");
    expect(tools).toHaveProperty("search_meeting_content");
    expect(tools).toHaveProperty("read_folder_material");
    expect(tools).toHaveProperty("list_folders");
    expect(tools).toHaveProperty("create_folder");
    expect(tools).toHaveProperty("move_meetings_to_folder");
    expect(tools).toHaveProperty("find_related_meetings");
    expect(tools).toHaveProperty("edit_memo");
    expect(tools).toHaveProperty("edit_summary");
    expect(tools).toHaveProperty("move_meeting_contents");
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
    const tools = buildChatTools(dependencies(), access);

    await (tools.get_meeting as any).execute({
      meeting_id: "private input",
    });

    expect(JSON.stringify(log.mock.calls)).not.toContain("private");
  });

  it("does not log meeting tool errors", async () => {
    vi.mocked(getMeeting).mockRejectedValue(new Error("private error"));
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const tools = buildChatTools(dependencies(), access);

    await expect(
      (tools.get_meeting as any).execute({ meeting_id: "private input" }),
    ).rejects.toThrow("private error");

    expect(JSON.stringify(error.mock.calls)).not.toContain("private");
  });

  it.each([
    "get_recurring_meeting_history",
    "find_related_meetings",
    "read_folder_material",
    "web_search",
  ] as const)("blocks %s for Free before executing it", async (name) => {
    const tools = buildChatTools(dependencies(), {
      canUsePro: () => false,
      onProRequired: vi.fn(),
    });
    await expect((tools[name] as any).execute({})).rejects.toThrow(
      "This requires Anarlog Pro",
    );
  });

  it("rechecks advanced tool access on downgrade while leaving ordinary note access free", async () => {
    let isPro = true;
    const tools = buildChatTools(dependencies(), {
      canUsePro: () => isPro,
      onProRequired: vi.fn(),
    });
    const history = { meetings: [], pagination: { next_offset: null } };
    vi.mocked(getRecurringMeetingHistory).mockResolvedValue(history as never);
    const execute = tools.get_recurring_meeting_history.execute! as any;
    await expect(execute({ meeting_id: "meeting-1" })).resolves.toEqual(
      history,
    );
    isPro = false;
    await expect(execute({ meeting_id: "meeting-1" })).rejects.toThrow(
      "This requires Anarlog Pro",
    );
    vi.mocked(getMeeting).mockResolvedValue({ title: "Local note" } as never);
    await expect(
      (tools.get_meeting as any).execute({ meeting_id: "meeting-1" }),
    ).resolves.toEqual({ title: "Local note" });
  });
});
