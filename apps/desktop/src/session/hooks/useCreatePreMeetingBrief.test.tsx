import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PastSessionNote } from "~/session/insights/past-notes";

const mocks = vi.hoisted(() => ({
  event: {
    title: "Weekly Product Sync",
    started_at: "2026-08-21T09:00:00.000Z",
    ended_at: "2026-08-21T10:00:00.000Z",
    is_all_day: false,
    description: "Review the launch plan.",
    participants: [{ name: "Ada" }],
  } as Record<string, unknown> | null,
  now: new Date("2026-08-21T08:00:00.000Z"),
  model: { id: "model-1" } as { id: string } | null,
  notes: [] as PastSessionNote[],
  rawMd: "",
  title: "",
  userId: "me",
  participants: [] as Array<{
    humanId: string;
    source: string;
    name: string;
    email: string;
  }>,
  streamPreMeetingBrief: vi.fn(),
  updateSession: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => mocks.model,
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => mocks.now,
}));

vi.mock("~/calendar/queries", () => ({
  useSessionCalendarEvent: () => mocks.event,
}));

vi.mock("~/session/insights/past-notes", () => ({
  usePastSessionNotes: () => ({
    notes: mocks.notes,
    hasPastNotes: mocks.notes.length > 0,
    isGenerating: false,
    canGenerate: true,
    regenerate: vi.fn(),
    regenerateAll: vi.fn(),
  }),
}));

vi.mock("~/session/insights/pre-meeting", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/session/insights/pre-meeting")>()),
  streamPreMeetingBrief: mocks.streamPreMeetingBrief,
}));

vi.mock("~/session/queries", () => ({
  useSession: () => ({
    raw_md: mocks.rawMd,
    title: mocks.title,
    user_id: mocks.userId,
  }),
  useSessionParticipants: () => mocks.participants,
  updateSession: mocks.updateSession,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => "en",
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("@anlg/editor/markdown", () => ({
  md2json: (markdown: string) => ({ type: "doc", content: [{ markdown }] }),
}));

import { resetPreMeetingBriefJobs } from "./pre-meeting-brief-job";
import { useCreatePreMeetingBrief } from "./useCreatePreMeetingBrief";

function renderBriefHook(
  overrides: Partial<Parameters<typeof useCreatePreMeetingBrief>[0]> = {},
) {
  return renderHook(() =>
    useCreatePreMeetingBrief({
      sessionId: "current",
      sessionMode: "inactive",
      isMemoView: true,
      onSwitchToMemos: () => {},
      getMemoEditor: () => null,
      ...overrides,
    }),
  );
}

describe("useCreatePreMeetingBrief", () => {
  beforeEach(() => {
    mocks.event = {
      title: "Weekly Product Sync",
      started_at: "2026-08-21T09:00:00.000Z",
      ended_at: "2026-08-21T10:00:00.000Z",
      is_all_day: false,
      description: "Review the launch plan.",
      participants: [{ name: "Ada" }],
    };
    mocks.now = new Date("2026-08-21T08:00:00.000Z");
    mocks.model = { id: "model-1" };
    mocks.notes = [
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "Aug 14, 2026",
        occurredAt: "2026-08-14T09:00:00.000Z",
        sourceSummary: "Ada will share the prototype.",
        relationship: "same_series",
        summary: "Ada will share the prototype.",
        isGenerating: false,
      },
    ];
    mocks.rawMd = "";
    mocks.title = "";
    mocks.userId = "me";
    mocks.participants = [];
    mocks.streamPreMeetingBrief.mockReset();
    mocks.streamPreMeetingBrief.mockImplementation(
      async ({ onText }: { onText?: (text: string) => void }) => {
        onText?.("## Brief");
        return "## Brief";
      },
    );
    mocks.updateSession.mockReset();
    mocks.updateSession.mockResolvedValue(undefined);
    mocks.toastError.mockReset();
  });

  afterEach(() => {
    resetPreMeetingBriefJobs();
    cleanup();
  });

  it("is available only for upcoming meetings with prior notes", () => {
    const { result, rerender } = renderBriefHook();

    expect(result.current.visible).toBe(true);

    mocks.notes = [];
    rerender();
    expect(result.current.visible).toBe(false);
  });

  it("shows create brief on untitled notes after participants are added", () => {
    mocks.event = null;
    mocks.title = "";

    const { result, rerender } = renderBriefHook();

    expect(result.current.visible).toBe(false);

    mocks.participants = [
      {
        humanId: "yujong",
        source: "manual",
        name: "Yujong Lee",
        email: "yujonglee@fastrepl.com",
      },
    ];
    rerender();
    expect(result.current.visible).toBe(true);
  });

  it("hides after the memo has content and returns when the memo is cleared", () => {
    const { result, rerender } = renderBriefHook();

    expect(result.current.visible).toBe(true);

    mocks.rawMd = "## Brief\n\nAda will share the prototype.";
    rerender();
    expect(result.current.visible).toBe(false);

    mocks.rawMd = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
    rerender();
    expect(result.current.visible).toBe(true);
  });

  it("shows create brief as soon as the live memo is empty", () => {
    mocks.rawMd = "## Brief\n\nAda will share the prototype.";

    const { result, rerender } = renderHook(
      ({ isMemoEmpty }) =>
        useCreatePreMeetingBrief({
          sessionId: "current",
          sessionMode: "inactive",
          isMemoView: true,
          isMemoEmpty,
          onSwitchToMemos: () => {},
          getMemoEditor: () => null,
        }),
      { initialProps: { isMemoEmpty: false } },
    );

    expect(result.current.visible).toBe(false);

    rerender({ isMemoEmpty: true });
    expect(result.current.visible).toBe(true);
  });

  it("writes the generated brief into the memo editor", async () => {
    const replaceContent = vi.fn();
    const flushPendingChanges = vi.fn();
    const onSwitchToMemos = vi.fn();

    const { result } = renderBriefHook({
      onSwitchToMemos,
      getMemoEditor: () => ({ replaceContent, flushPendingChanges }),
    });

    act(() => {
      result.current.createBrief();
    });

    await waitFor(() => {
      expect(onSwitchToMemos).toHaveBeenCalledOnce();
      expect(replaceContent).toHaveBeenCalledWith({
        type: "doc",
        content: [{ markdown: "## Brief" }],
      });
      expect(flushPendingChanges).toHaveBeenCalledOnce();
    });
  });

  it("persists the brief when the memo editor is not mounted", async () => {
    const { result } = renderBriefHook();

    act(() => {
      result.current.createBrief();
    });

    await waitFor(() => {
      expect(mocks.updateSession).toHaveBeenCalledWith("current", {
        raw_md: JSON.stringify({
          type: "doc",
          content: [{ markdown: "## Brief" }],
        }),
      });
    });
  });

  it("keeps generating after leaving the note and resumes in the remounted editor", async () => {
    let releaseStream: (() => void) | undefined;
    mocks.streamPreMeetingBrief.mockImplementation(
      async ({ onText }: { onText?: (text: string) => void }) => {
        onText?.("partial");
        await new Promise<void>((resolve) => {
          releaseStream = resolve;
        });
        onText?.("final");
        return "final";
      },
    );

    const { result, unmount } = renderBriefHook();

    act(() => {
      result.current.createBrief();
    });

    expect(result.current.isGenerating).toBe(true);
    await waitFor(() => {
      expect(mocks.updateSession).toHaveBeenCalledWith("current", {
        raw_md: JSON.stringify({
          type: "doc",
          content: [{ markdown: "partial" }],
        }),
      });
    });

    unmount();

    const replaceContent = vi.fn();
    const flushPendingChanges = vi.fn();
    const { result: otherNote } = renderBriefHook({ sessionId: "other" });
    expect(otherNote.current.isGenerating).toBe(false);

    const { result: resumed } = renderBriefHook({
      getMemoEditor: () => ({ replaceContent, flushPendingChanges }),
    });
    expect(resumed.current.isGenerating).toBe(true);
    expect(resumed.current.visible).toBe(true);

    await act(async () => {
      releaseStream?.();
    });

    await waitFor(() => {
      expect(resumed.current.isGenerating).toBe(false);
      expect(replaceContent).toHaveBeenCalledWith({
        type: "doc",
        content: [{ markdown: "final" }],
      });
      expect(flushPendingChanges).toHaveBeenCalledOnce();
    });
  });
});
