import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PastSessionNote } from "./past-notes";
import {
  canCreatePreMeetingBrief,
  compactBriefText,
  formatPreMeetingBrief,
  getPreMeetingBriefFacts,
  mergeBriefMarkdown,
  selectBriefSourceNotes,
  shouldShowPreMeetingBrief,
  streamPreMeetingBrief,
  trimPreMeetingBrief,
} from "./pre-meeting";

const hoisted = vi.hoisted(() => ({
  renderCustom: vi.fn(async (_template: string, ctx: unknown) => ({
    status: "ok" as const,
    data: JSON.stringify(ctx),
  })),
  streamText: vi.fn(),
}));

vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  streamText: hoisted.streamText,
}));

vi.mock("@anlg/plugin-template", () => ({
  commands: {
    renderCustom: hoisted.renderCustom,
  },
}));

function makeNote(
  overrides: Partial<PastSessionNote> & Pick<PastSessionNote, "sessionId">,
): PastSessionNote {
  return {
    title: "Weekly sync",
    dateLabel: "Aug 14, 2026",
    occurredAt: "2026-08-14T09:00:00.000Z",
    sourceSummary: "The source summary remains available.",
    relationship: "same_series",
    summary: "- Confirm launch timing.\n- Ada owns the prototype.",
    isGenerating: false,
    ...overrides,
  };
}

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
    const note = makeNote({ sessionId: "previous" });

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

describe("pre-meeting brief visibility", () => {
  const nowMs = Date.parse("2026-08-21T08:00:00.000Z");

  it("stays hidden for past and all-day events", () => {
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-21T07:00:00.000Z",
          ended_at: "2026-08-21T07:30:00.000Z",
          is_all_day: false,
        },
        nowMs,
      ),
    ).toBe(false);
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-21T07:58:00.000Z",
          ended_at: "",
          is_all_day: false,
        },
        nowMs,
      ),
    ).toBe(true);
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-22T07:00:00.000Z",
          ended_at: "2026-08-22T07:30:00.000Z",
          is_all_day: true,
        },
        nowMs,
      ),
    ).toBe(false);
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-21T08:02:00.0000000",
          ended_at: "2026-08-21T09:00:00.0000000",
          is_all_day: false,
        },
        nowMs,
      ),
    ).toBe(true);
  });

  it("requires an upcoming event or added participants, plus usable prior meetings", () => {
    const event = {
      started_at: "2026-08-21T09:00:00.000Z",
      ended_at: "2026-08-21T10:00:00.000Z",
      is_all_day: false,
    };
    const notes = [makeNote({ sessionId: "previous" })];

    expect(canCreatePreMeetingBrief({ event, nowMs, notes })).toBe(true);
    expect(canCreatePreMeetingBrief({ event, nowMs, notes: [] })).toBe(false);
    expect(
      canCreatePreMeetingBrief({
        event: null,
        nowMs,
        notes,
      }),
    ).toBe(false);
    expect(
      canCreatePreMeetingBrief({
        event: null,
        nowMs,
        notes,
        hasParticipants: true,
      }),
    ).toBe(true);
    expect(
      canCreatePreMeetingBrief({
        event: null,
        nowMs,
        notes: [],
        hasParticipants: true,
      }),
    ).toBe(false);
  });
});

describe("brief source notes", () => {
  it("keeps the five most recent meetings with usable notes", () => {
    const notes = Array.from({ length: 6 }, (_, index) =>
      makeNote({
        sessionId: `meeting-${index}`,
        title: `Meeting ${index}`,
        sourceSummary: index === 2 ? "" : `Notes from meeting ${index}`,
        summary: index === 2 ? null : `Fact ${index}`,
      }),
    );

    expect(selectBriefSourceNotes(notes).map((note) => note.sessionId)).toEqual(
      ["meeting-0", "meeting-1", "meeting-3", "meeting-4", "meeting-5"],
    );
  });
});

describe("formatPreMeetingBrief", () => {
  it("formats a JSON opener and three bullets", () => {
    expect(
      formatPreMeetingBrief({
        opener: "Ada slipped the prototype date.",
        bullets: [
          "John still owns the scratchpad rewrite.",
          "CI cost gating is unresolved.",
          "Artem left the Korea workshop dates open.",
          "Extra fact should not appear.",
        ],
      }),
    ).toBe(`**Ada slipped the prototype date.**

- John still owns the scratchpad rewrite.
- CI cost gating is unresolved.
- Artem left the Korea workshop dates open.`);
  });

  it("drops leftover instructions from structured fields", () => {
    expect(
      formatPreMeetingBrief({
        opener: "One sentence: why this conversation matters.",
        bullets: [
          "John's proposal to show a Linear chip above the chat box.",
          "Yujong's commitment to discuss CI spending.",
        ],
      }),
    ).toBe(`- John's proposal to show a Linear chip above the chat box.
- Yujong's commitment to discuss CI spending.`);
  });
});

describe("trimPreMeetingBrief", () => {
  it("keeps a one-liner and the first three bullets", () => {
    expect(
      trimPreMeetingBrief(`**Ada slipped the prototype date.**

- John still owns the scratchpad rewrite.
- CI cost gating is unresolved.
- Artem left the Korea workshop dates open.
- Extra fact should not appear.
`),
    ).toBe(`**Ada slipped the prototype date.**

- John still owns the scratchpad rewrite.
- CI cost gating is unresolved.
- Artem left the Korea workshop dates open.`);
  });

  it("drops a meeting-justification opener and keeps real bullets", () => {
    expect(
      trimPreMeetingBrief(`**Design Sync is crucial for aligning the team's vision.**

- Artem and John will discuss the single-surface scratchpad.
- John will report on suggestions UI.
- John will propose a Linear ticket chip.
`),
    ).toBe(`- Artem and John will discuss the single-surface scratchpad.
- John will report on suggestions UI.
- John will propose a Linear ticket chip.`);
  });

  it("drops copied prompt instructions and keeps real bullets", () => {
    expect(
      trimPreMeetingBrief(`**One sentence: why this conversation matters.**

- John's proposal to show a Linear chip above the chat box.
- Yujong's commitment to discuss CI spending.
- Artem's proposed workshop dates for October 6th-10th.
`),
    ).toBe(`- John's proposal to show a Linear chip above the chat box.
- Yujong's commitment to discuss CI spending.
- Artem's proposed workshop dates for October 6th-10th.`);
  });

  it("drops a recap list and its mirrored preview", () => {
    expect(
      trimPreMeetingBrief(`Quick Recap for Founders Sync Meeting:
- John proposed a single-surface scratchpad.
- Sungbin has been focusing on Linear tickets.
- John raised CI cost gating.
- Artem mentioned October workshop dates.
- John asked Granola about the waitlist.

Upcoming Meeting Insight:
- Expect John to discuss the scratchpad.
- Sungbin might present Linear progress.
`),
    ).toBe(`- John proposed a single-surface scratchpad.
- Sungbin has been focusing on Linear tickets.
- John raised CI cost gating.`);
  });
});

describe("mergeBriefMarkdown", () => {
  it("replaces empty memos and prepends when notes already exist", () => {
    expect(mergeBriefMarkdown("## Brief", "")).toBe("## Brief");
    expect(mergeBriefMarkdown("## Brief", "Existing notes")).toBe(
      "## Brief\n\nExisting notes",
    );
  });
});

describe("streamPreMeetingBrief", () => {
  beforeEach(() => {
    hoisted.renderCustom.mockClear();
    hoisted.streamText.mockReset();
  });

  it("sends only a few facts so the model cannot list every thread twice", async () => {
    hoisted.streamText.mockReturnValue({
      partialOutputStream: (async function* () {
        yield { bullets: ["Follow up with Ada."] };
      })(),
      output: Promise.resolve({ bullets: ["Follow up with Ada."] }),
    });

    await streamPreMeetingBrief({
      model: { id: "model-1" } as never,
      language: "en",
      event: { title: "Weekly Product Sync" },
      notes: Array.from({ length: 3 }, (_, index) =>
        makeNote({
          sessionId: `meeting-${index}`,
          summary: `- Fact ${index}a.\n- Fact ${index}b.`,
        }),
      ),
    });

    const promptContext = hoisted.renderCustom.mock.calls.find((call) => {
      const ctx = call[1] as { past_meetings?: Array<{ notes: string }> };
      return Array.isArray(ctx.past_meetings);
    })?.[1] as { past_meetings: Array<{ notes: string }> };

    expect(
      promptContext.past_meetings.flatMap((meeting) =>
        meeting.notes.split("\n"),
      ),
    ).toEqual(["Fact 0a.", "Fact 0b.", "Fact 1a.", "Fact 1b."]);
  });

  it("streams a brief from the latest related meetings", async () => {
    hoisted.streamText.mockReturnValue({
      partialOutputStream: (async function* () {
        yield { opener: "Follow up with Ada on launch timing." };
        yield {
          opener: "Follow up with Ada on launch timing.",
          bullets: ["Ada owns the prototype."],
        };
      })(),
      output: Promise.resolve({
        opener: "Follow up with Ada on launch timing.",
        bullets: ["Ada owns the prototype."],
      }),
    });

    const chunks: string[] = [];
    const text = await streamPreMeetingBrief({
      model: { id: "model-1" } as never,
      language: "en",
      event: {
        title: "Weekly Product Sync",
        started_at: "2026-08-21T09:00:00.000Z",
        ended_at: "2026-08-21T10:00:00.000Z",
        description: "Review the launch plan.",
        participants: [{ name: "Ada" }],
      },
      notes: [makeNote({ sessionId: "previous" })],
      onText: (value) => chunks.push(value),
    });

    expect(text).toBe(
      "**Follow up with Ada on launch timing.**\n\n- Ada owns the prototype.",
    );
    expect(chunks).toEqual([
      "**Follow up with Ada on launch timing.**",
      "**Follow up with Ada on launch timing.**\n\n- Ada owns the prototype.",
    ]);
    expect(hoisted.streamText).toHaveBeenCalledWith(
      expect.objectContaining({
        model: { id: "model-1" },
        maxOutputTokens: 200,
        output: expect.objectContaining({}),
      }),
    );
    expect(hoisted.renderCustom).toHaveBeenCalled();
  });
});
