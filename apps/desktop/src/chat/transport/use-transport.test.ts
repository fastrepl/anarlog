import { describe, expect, it } from "vitest";

import { appendMeetingContextToolGuidance } from "./use-transport";

describe("chat transport prompt guidance", () => {
  it("tells chat to use typed meeting search tools", () => {
    const prompt = appendMeetingContextToolGuidance("Base prompt");

    expect(prompt).toContain("Base prompt");
    expect(prompt).toContain("Use list_meetings");
    expect(prompt).toContain("Use search_meetings");
    expect(prompt).toContain("Use search_meeting_content");
    expect(prompt).toContain("use get_meeting");
    expect(prompt).toContain("Use get_meeting_transcript");
    expect(prompt).toContain("Use get_recurring_meeting_history");
    expect(prompt).toContain("Use typed meeting tools");
    expect(prompt).toContain("Do not ask the user to open or share a meeting");
    expect(prompt).not.toContain("grep_notes");
    expect(prompt).not.toContain("search_sessions");
    expect(prompt).not.toContain("read_note");
    expect(prompt).not.toContain("read_current_note");
  });
});
