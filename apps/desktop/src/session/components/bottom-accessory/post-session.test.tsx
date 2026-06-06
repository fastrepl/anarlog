import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { buildPastSessionNotes } from "./past-notes";
import { PostSessionAccessory } from "./post-session";

const {
  audioPathMock,
  useTranscriptScreenMock,
  useRunBatchMock,
  useListenerMock,
  useTranscriptExportSegmentsMock,
  runBatchMock,
  handleBatchFailedMock,
  writeTextMock,
  showTransientToastMock,
} = vi.hoisted(() => ({
  audioPathMock: vi.fn(),
  useTranscriptScreenMock: vi.fn(),
  useRunBatchMock: vi.fn(),
  useListenerMock: vi.fn(),
  useTranscriptExportSegmentsMock: vi.fn(),
  runBatchMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
  writeTextMock: vi.fn(),
  showTransientToastMock: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioPath: audioPathMock,
  },
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
  Timeline: () => <div data-testid="timeline" />,
  TimelineShell: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TimelineMeta: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  useAudioPlayer: () => ({
    audioExists: true,
    deleteRecording: vi.fn(),
    isDeletingRecording: false,
  }),
}));

vi.mock("~/session/components/note-input/transcript", () => ({
  Transcript: () => <div data-testid="transcript" />,
}));

vi.mock("~/sidebar/toast/transient", () => ({
  showTransientToast: showTransientToastMock,
}));

vi.mock("~/session/components/note-input/transcript/export-data", () => ({
  useTranscriptExportSegments: useTranscriptExportSegmentsMock,
  formatTranscriptExportSegments: (
    segments: Array<{ speaker: string | null; text: string }>,
  ) =>
    segments
      .map((segment) => `${segment.speaker ?? "Speaker"}: ${segment.text}`)
      .join("\n\n"),
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

vi.mock("~/stt/useRunBatch", () => ({
  useRunBatch: useRunBatchMock,
  isStoppedTranscriptionError: vi.fn(() => false),
}));

describe("PostSessionAccessory", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: writeTextMock,
      },
    });

    audioPathMock.mockResolvedValue({
      status: "ok",
      data: "/tmp/session.wav",
    });

    useTranscriptScreenMock.mockReturnValue({
      kind: "ready",
      transcriptIds: ["transcript-1"],
      liveSegments: [],
      currentActive: false,
    });
    useTranscriptExportSegmentsMock.mockReturnValue({
      data: [
        { speaker: "Alex", text: "We should ship this." },
        { speaker: null, text: "Agreed." },
      ],
      isLoading: false,
    });

    writeTextMock.mockResolvedValue(undefined);
    runBatchMock.mockResolvedValue(undefined);
    useRunBatchMock.mockReturnValue(runBatchMock);

    useListenerMock.mockImplementation((selector) =>
      selector({
        handleBatchFailed: handleBatchFailedMock,
        stopTranscription: vi.fn(),
      }),
    );
  });

  it("starts regeneration without local batch state bookkeeping", async () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => {
      expect(audioPathMock).toHaveBeenCalledWith("session-1");
      expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    });

    expect(handleBatchFailedMock).not.toHaveBeenCalled();
  });

  it("copies transcript text from the expanded transcript panel", async () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Copy transcript" }));

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith(
        "Alex: We should ship this.\n\nSpeaker: Agreed.",
      );
    });
    expect(showTransientToastMock).toHaveBeenCalledWith({
      id: "transcript-copy-success",
      description: "Transcript copied to clipboard",
    });
  });

  it("shows Regenerate button without upload or reserved height in empty panel", async () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "ready",
      transcriptIds: [],
      liveSegments: [],
      currentActive: false,
    });

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript={false}
        isTranscriptExpanded
        fillHeight
      />,
    );

    const noTranscript = screen.getByText("No transcript yet");
    const transcriptCard = noTranscript.parentElement?.parentElement;
    const transcriptSlot = transcriptCard?.parentElement;
    expect(transcriptCard?.className).not.toContain("min-h-[114px]");
    expect(transcriptCard?.className).not.toContain("min-h-[96px]");
    expect(transcriptSlot?.className).not.toContain("min-h-[114px]");
    expect(transcriptSlot?.className).toContain("shrink-0");
    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Regenerate/ }));

    await waitFor(() => {
      expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    });
  });

  it("keeps the audio timeline visible while the transcript panel is collapsed", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded={false}
      />,
    );

    expect(screen.getByTestId("timeline")).toBeTruthy();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });

  it("keeps the audio timeline slot height stable between collapsed and expanded states", () => {
    const { unmount } = render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded={false}
      />,
    );

    const collapsedSlotClassName =
      screen.getByTestId("timeline").parentElement?.className;
    expect(collapsedSlotClassName).toContain("h-10");
    expect(collapsedSlotClassName).toContain("-mt-1.5");

    unmount();

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
        fillHeight
      />,
    );

    const expandedSlotClassName =
      screen.getByTestId("timeline").parentElement?.className;
    expect(expandedSlotClassName).toContain("h-10");
    expect(expandedSlotClassName).not.toContain("-mt-1.5");
  });

  it("lets expanded transcript content fill the resizable bottom panel", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
        fillHeight
      />,
    );

    const scrollArea = screen.getByTestId("transcript").parentElement;
    expect(scrollArea?.className).toContain("flex-1");
    expect(scrollArea?.className).not.toContain("h-[300px]");

    const transcriptCard = scrollArea?.parentElement;
    const transcriptSlot = transcriptCard?.parentElement;
    expect(transcriptCard?.className).toContain("rounded-b-xl");
    expect(transcriptCard?.className).toContain("border");
    expect(transcriptCard?.className).toContain("h-full");
    expect(transcriptCard?.className).toContain("min-h-[114px]");
    expect(transcriptCard?.className).not.toContain("min-h-[96px]");
    expect(transcriptSlot?.className).toContain("flex-1");
    expect(transcriptSlot?.className).toContain("min-h-[114px]");
  });

  it("shows transcript skeletons instead of duplicating batch progress in the body", () => {
    useTranscriptScreenMock.mockReturnValue({
      kind: "running_batch",
      percentage: 0.25,
      phase: "transcribing",
    });

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
      />,
    );

    expect(screen.getByText("Transcript")).toBeTruthy();
    expect(screen.getAllByText("Transcribing...")).toHaveLength(1);
    expect(screen.getAllByTestId("spinner")).toHaveLength(1);
    expect(screen.getByTestId("transcript-skeleton")).toBeTruthy();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });

  it("renders the past notes timeline when the past notes tab is active", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio={false}
        hasTranscript
        isTranscriptExpanded
        activeTab="past_notes"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            summary:
              "- Ship the transcript panel.\nRevisit visual polish next week.\n3. Confirm keyboard behavior.\nDo not show this fourth fact.",
            isGenerating: false,
          },
        ]}
      />,
    );

    expect(screen.getByText("Related meetings")).toBeTruthy();
    const title = screen.getByText("Weekly Product Sync");
    const date = screen.getByText("May 28, 2026");
    expect(title).toBeTruthy();
    expect(date).toBeTruthy();
    expect(date.compareDocumentPosition(title)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    const factsList = screen.getByRole("list");
    expect(factsList.className).toContain("list-disc");
    expect(factsList.className).toContain("list-inside");
    expect(factsList.className).not.toContain("flex");
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
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
        summary: null,
        isGenerating: false,
      },
      {
        sessionId: "same_title",
        title: "Weekly Product Sync",
        dateLabel: "May 27, 2026",
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
