import { describe, expect, it } from "vitest";

import { compactBriefText, getPreMeetingBriefFacts } from "./pre-meeting";

describe("pre-meeting brief text", () => {
  it("keeps useful link labels without exposing raw URLs", () => {
    expect(
      compactBriefText(
        "Review [launch plan](https://example.com/launch) at https://meet.example.com.",
        200,
      ),
    ).toBe("Review launch plan at");
  });

  it("prefers generated facts and falls back to the source summary", () => {
    const note = {
      sessionId: "previous",
      title: "Weekly sync",
      dateLabel: "Aug 14, 2026",
      occurredAt: "2026-08-14T09:00:00.000Z",
      participantNames: ["Ada"],
      sourceSummary: "The source summary remains available.",
      relationship: "same_series" as const,
      summary: "- Confirm launch timing.\n- Ada owns the prototype.",
      isGenerating: false,
    };

    expect(getPreMeetingBriefFacts(note)).toEqual([
      "Confirm launch timing.",
      "Ada owns the prototype.",
    ]);
    expect(
      getPreMeetingBriefFacts({
        ...note,
        summary: null,
      }),
    ).toEqual(["The source summary remains available."]);
  });
});
