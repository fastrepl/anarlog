import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { buildPastSessionNotes } from "./past-notes";
import { PostSessionAccessory } from "./post-session";

const { useTranscriptScreenMock, useListenerMock } = vi.hoisted(() => ({
  useTranscriptScreenMock: vi.fn(),
  useListenerMock: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <div data-testid="spinner" />,
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("~/audio-player", () => ({
  TimelineShell: ({
    children,
    leading,
    main,
    meta,
  }: {
    children?: React.ReactNode;
    leading?: React.ReactNode;
    main?: React.ReactNode;
    meta?: React.ReactNode;
  }) => (
    <div>
      {leading}
      {main}
      {meta}
      {children}
    </div>
  ),
  TimelineMeta: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/session/components/note-input/transcript/state", () => ({
  useTranscriptScreen: useTranscriptScreenMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  UI: {
    useStore: vi.fn(() => null),
    useIndexes: vi.fn(() => null),
    useTable: vi.fn(() => ({})),
    useValue: vi.fn(() => null),
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

describe("PostSessionAccessory", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    useTranscriptScreenMock.mockReturnValue({
      kind: "ready",
      transcriptIds: ["transcript-1"],
      liveSegments: [],
      currentActive: false,
    });

    useListenerMock.mockImplementation((selector) =>
      selector({
        stopTranscription: vi.fn(),
      }),
    );
  });

  it("does not render a bottom transcript panel for ready transcripts", () => {
    const { container } = render(
      <PostSessionAccessory sessionId="session-1" isTranscriptExpanded />,
    );

    expect(container.textContent).toBe("");
    expect(screen.queryByRole("button", { name: "Regenerate" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Copy transcript" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Delete recording" }),
    ).toBeNull();
  });

  it("keeps batch status visible while the transcript panel is collapsed", () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "running_batch",
      percentage: 0.25,
      phase: "transcribing",
    });

    render(
      <PostSessionAccessory
        sessionId="session-1"
        isTranscriptExpanded={false}
      />,
    );

    expect(screen.queryByText("25%")).toBeNull();
    expect(screen.getByText("Transcribing...")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Stop transcription" }),
    ).toBeTruthy();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });

  it("shows uploading state without a stop action while importing audio", () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "running_batch",
      percentage: 0.25,
      phase: "importing",
    });

    render(
      <PostSessionAccessory
        sessionId="session-1"
        isTranscriptExpanded={false}
      />,
    );

    expect(screen.getByText("Uploading...")).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Stop transcription" }),
    ).toBeNull();
    expect(screen.queryByText("25%")).toBeNull();
  });

  it("shows compact batch status without transcript body skeletons", () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "running_batch",
      percentage: 0.25,
      phase: "transcribing",
    });

    render(<PostSessionAccessory sessionId="session-1" isTranscriptExpanded />);

    expect(screen.queryByText("Transcript")).toBeNull();
    expect(screen.getByText("Transcribing...")).toBeTruthy();
    expect(screen.queryByText("25%")).toBeNull();
    expect(screen.getByTestId("spinner")).toBeTruthy();
  });

  it("renders compiled insights when the insights tab is active", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        isTranscriptExpanded
        activeTab="insights"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            participantNames: ["Alex", "Jamie"],
            summary:
              "- Ship the transcript panel.\nRevisit visual polish next week.\n3. Confirm keyboard behavior.\nDo not show this fourth fact.",
            isGenerating: false,
          },
        ]}
      />,
    );

    expect(screen.getByText("Insights")).toBeTruthy();
    expect(screen.getByText("Alex")).toBeTruthy();
    expect(screen.getByText("Jamie")).toBeTruthy();
    expect(screen.queryByText("Weekly Product Sync")).toBeNull();
    expect(screen.queryByText("May 28, 2026")).toBeNull();
    const factsList = screen.getByRole("list");
    expect(factsList.className).toContain("list-disc");
    expect(factsList.className).not.toContain("list-inside");
    expect(factsList.className).toContain("pl-5");
    expect(factsList.className).not.toContain("overflow-hidden");
    const facts = screen.getAllByRole("listitem");
    expect(facts).toHaveLength(3);
    expect(facts[0].className).not.toContain("line-clamp");
    expect(facts[0].querySelector(".line-clamp-2")).toBeNull();
    expect(screen.getByText("Ship the transcript panel.")).toBeTruthy();
    expect(screen.getByText("Revisit visual polish next week.")).toBeTruthy();
    expect(screen.getByText("Confirm keyboard behavior.")).toBeTruthy();
    expect(screen.queryByText("Do not show this fourth fact.")).toBeNull();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });

  it("builds descending past notes from recurring and same-title sessions", () => {
    const store = makeStore({
      sessions: {
        current: {
          title: "Weekly Product Sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-06-03T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
          raw_md: "",
        },
        previous: {
          title: "Weekly Product Sync",
          created_at: "2026-05-28T10:00:00.000Z",
          event_json: JSON.stringify({
            started_at: "2026-05-28T10:00:00.000Z",
            recurrence_series_id: "series-1",
          }),
          raw_md: "",
        },
        same_title: {
          title: "Weekly Product Sync",
          created_at: "2026-05-27T10:00:00.000Z",
          event_json: "",
          raw_md: "Confirmed notification copy and reviewed follow-ups.",
        },
        older: {
          title: "Older Product Sync",
          created_at: "2026-05-21T10:00:00.000Z",
          event_json: "",
          raw_md: "Reviewed onboarding follow-ups and assigned owners.",
        },
        partial: {
          title: "Alex 1:1",
          created_at: "2026-05-20T10:00:00.000Z",
          event_json: "",
          raw_md: "Should not show up.",
        },
        future: {
          title: "Future Product Sync",
          created_at: "2026-06-10T10:00:00.000Z",
          event_json: "",
          raw_md: "Should not show up.",
        },
      },
      mapping_session_participant: {
        current_self: {
          session_id: "current",
          human_id: "self",
          user_id: "self",
          source: "manual",
        },
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        current_jamie: {
          session_id: "current",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        previous_alex: {
          session_id: "previous",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        previous_jamie: {
          session_id: "previous",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        same_title_alex: {
          session_id: "same_title",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        same_title_jamie: {
          session_id: "same_title",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        older_alex: {
          session_id: "older",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        older_jamie: {
          session_id: "older",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        partial_alex: {
          session_id: "partial",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        future_alex: {
          session_id: "future",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        future_jamie: {
          session_id: "future",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
      },
      enhanced_notes: {
        previous_summary: {
          session_id: "previous",
          content:
            "Aligned on transcript panel behavior. Past notes should stay short and scannable.",
          position: 0,
        },
      },
    });

    const result = buildPastSessionNotes(store, "current", "self");

    expect(result.notes).toEqual([
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        participantNames: ["alex", "jamie"],
        summary: null,
        isGenerating: false,
      },
      {
        sessionId: "same_title",
        title: "Weekly Product Sync",
        dateLabel: "May 27, 2026",
        participantNames: ["alex", "jamie"],
        summary: null,
        isGenerating: false,
      },
    ]);
    expect(result.missing.map((request) => request.sessionId)).toEqual([
      "previous",
      "same_title",
    ]);
  });

  it("does not treat matching participants alone as related past notes", () => {
    const store = makeStore({
      sessions: {
        current: {
          title: "Design sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: "",
          raw_md: "",
        },
        different_topic: {
          title: "Dev sync",
          created_at: "2026-06-01T10:00:00.000Z",
          event_json: "",
          raw_md: "Discussed release branch status.",
        },
      },
      mapping_session_participant: {
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        current_jamie: {
          session_id: "current",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
        different_topic_alex: {
          session_id: "different_topic",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        different_topic_jamie: {
          session_id: "different_topic",
          human_id: "jamie",
          user_id: "self",
          source: "auto",
        },
      },
    });

    const result = buildPastSessionNotes(store, "current", "self");

    expect(result.notes).toEqual([]);
    expect(result.missing).toEqual([]);
  });

  it("reuses saved key facts when the source hash still matches", () => {
    const store = makeStore({
      sessions: {
        current: {
          title: "Weekly Product Sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: "",
          raw_md: "",
        },
        previous: {
          title: "Weekly Product Sync",
          created_at: "2026-05-28T10:00:00.000Z",
          event_json: "",
          raw_md: "Alex committed to send pricing by Friday.",
        },
      },
      mapping_session_participant: {
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        previous_alex: {
          session_id: "previous",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
      },
    });

    const first = buildPastSessionNotes(store, "current", "self");
    expect(first.notes[0]?.summary).toBeNull();
    const request = first.missing[0]!;

    store.setRow("session_key_facts", "previous", {
      user_id: "self",
      session_id: "previous",
      created_at: "2026-05-28T11:00:00.000Z",
      updated_at: "2026-05-28T11:00:00.000Z",
      content: "Alex committed to send pricing by Friday.",
      source_hash: request.sourceHash,
    });

    const second = buildPastSessionNotes(store, "current", "self");

    expect(second.notes).toEqual([
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        participantNames: ["alex"],
        summary: "Alex committed to send pricing by Friday.",
        isGenerating: false,
      },
    ]);
    expect(second.missing).toHaveLength(0);
  });
});

function makeStore(
  tables: Record<string, Record<string, Record<string, any>>>,
) {
  return {
    getRow: (tableId: string, rowId: string) => tables[tableId]?.[rowId] ?? {},
    getCell: (tableId: string, rowId: string, cellId: string) =>
      tables[tableId]?.[rowId]?.[cellId],
    forEachRow: (
      tableId: string,
      callback: (rowId: string, forEachCell: unknown) => void,
    ) => {
      for (const rowId of Object.keys(tables[tableId] ?? {})) {
        callback(rowId, () => {});
      }
    },
    setRow: (tableId: string, rowId: string, row: Record<string, any>) => {
      tables[tableId] = {
        ...(tables[tableId] ?? {}),
        [rowId]: row,
      };
    },
  } as any;
}
