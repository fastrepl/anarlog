import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
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
  useSession: () => ({ raw_md: mocks.rawMd }),
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

import { useCreatePreMeetingBrief } from "./useCreatePreMeetingBrief";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
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
    mocks.streamPreMeetingBrief.mockReset();
    mocks.streamPreMeetingBrief.mockImplementation(
      async ({ onText }: { onText?: (text: string) => void }) => {
        onText?.("## Brief");
        return "## Brief";
      },
    );
    mocks.updateSession.mockReset();
    mocks.toastError.mockReset();
  });

  afterEach(cleanup);

  it("is available only for upcoming meetings with prior notes", () => {
    const { result, rerender } = renderHook(
      () =>
        useCreatePreMeetingBrief({
          sessionId: "current",
          sessionMode: "inactive",
          isMemoView: true,
          onSwitchToMemos: () => {},
          getMemoEditor: () => null,
        }),
      { wrapper },
    );

    expect(result.current.visible).toBe(true);

    mocks.notes = [];
    rerender();
    expect(result.current.visible).toBe(false);
  });

  it("hides after the memo has content and returns when the memo is cleared", () => {
    const { result, rerender } = renderHook(
      () =>
        useCreatePreMeetingBrief({
          sessionId: "current",
          sessionMode: "inactive",
          isMemoView: true,
          onSwitchToMemos: () => {},
          getMemoEditor: () => null,
        }),
      { wrapper },
    );

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

  it("writes the generated brief into the memo editor", async () => {
    const replaceContent = vi.fn();
    const flushPendingChanges = vi.fn();
    const onSwitchToMemos = vi.fn();

    const { result } = renderHook(
      () =>
        useCreatePreMeetingBrief({
          sessionId: "current",
          sessionMode: "inactive",
          isMemoView: true,
          onSwitchToMemos,
          getMemoEditor: () => ({ replaceContent, flushPendingChanges }),
        }),
      { wrapper },
    );

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
});
