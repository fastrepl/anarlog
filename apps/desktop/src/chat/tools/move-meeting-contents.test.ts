import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  moveSessionContents: vi.fn(),
}));

vi.mock("~/session/move-contents", () => ({
  moveSessionContents: mocks.moveSessionContents,
}));

import { buildMoveMeetingContentsTool } from "./move-meeting-contents";

describe("move meeting contents chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.moveSessionContents.mockResolvedValue({
      status: "moved",
      sourceMeetingId: "source",
      targetMeetingId: "target",
      sourceTitle: "Standup",
      targetTitle: "Board",
      moved: {
        recording: true,
        transcripts: 1,
        summaries: 1,
        notes: true,
        actionItems: 0,
      },
    });
  });

  it("defaults the source meeting to the current session", async () => {
    const tool = buildMoveMeetingContentsTool({
      getSessionId: () => "source",
    });

    await expect(
      (tool as any).execute({ targetMeetingId: "target" }),
    ).resolves.toMatchObject({ status: "moved" });

    expect(mocks.moveSessionContents).toHaveBeenCalledWith({
      sourceSessionId: "source",
      targetSessionId: "target",
    });
  });

  it("requires an explicit source when no meeting is open", async () => {
    const tool = buildMoveMeetingContentsTool({
      getSessionId: () => undefined,
    });

    await expect(
      (tool as any).execute({ targetMeetingId: "target" }),
    ).resolves.toEqual({
      status: "error",
      message:
        "No source meeting selected. Provide sourceMeetingId explicitly when calling move_meeting_contents.",
    });
    expect(mocks.moveSessionContents).not.toHaveBeenCalled();
  });
});
