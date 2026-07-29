import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  eventFireAndForget: vi.fn().mockResolvedValue(undefined),
  loadSessionContentSnapshot: vi.fn(),
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    eventFireAndForget: mocks.eventFireAndForget,
  },
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

import {
  getMeetingNoteCompletionEvent,
  trackMeetingNoteCompletion,
} from "./meeting-note-analytics";

describe("meeting note analytics", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("classifies the onboarding demo as the welcome note", () => {
    expect(
      getMeetingNoteCompletionEvent({
        tracking_id: "anarlog-onboarding-demo-v1",
      }),
    ).toBe("welcome_meeting_note_completed");
  });

  it("classifies other sessions as real meeting notes", () => {
    expect(getMeetingNoteCompletionEvent(null)).toBe("meeting_note_completed");
    expect(
      getMeetingNoteCompletionEvent({ tracking_id: "calendar-event" }),
    ).toBe("meeting_note_completed");
  });

  it("tracks completion after loading the persisted session", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      event: { tracking_id: "anarlog-onboarding-demo-v1" },
    });

    await trackMeetingNoteCompletion("session-1");

    expect(mocks.eventFireAndForget).toHaveBeenCalledWith({
      event: "welcome_meeting_note_completed",
    });
  });

  it("does not track a deleted session", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(null);

    await trackMeetingNoteCompletion("session-1");

    expect(mocks.eventFireAndForget).not.toHaveBeenCalled();
  });
});
