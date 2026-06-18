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
  audioExistsMock,
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
  audioExistsMock: vi.fn(),
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
  useAudioPlayer: () => ({
    audioExists: audioExistsMock(),
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
    audioExistsMock.mockReturnValue(true);

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

  it("hides ready transcript regeneration when the recording is missing", () => {
    audioExistsMock.mockReturnValue(false);

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio={false}
        hasTranscript
        isTranscriptExpanded
      />,
    );

    expect(screen.queryByRole("button", { name: "Regenerate" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Delete recording" }),
    ).toBeNull();
  });

  it("shows an error toast when regeneration cannot find the recording", async () => {
    audioPathMock.mockResolvedValue({
      status: "error",
      error: "audio_path_not_found",
    });

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
      expect(showTransientToastMock).toHaveBeenCalledWith({
        id: "transcript-regenerate-audio-missing-session-1",
        description: "Recording not found. It may have been deleted.",
        variant: "error",
      });
    });
    expect(runBatchMock).not.toHaveBeenCalled();
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

  it("uses dark-aware colors for the delete recording action", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded
      />,
    );

    const deleteButton = screen.getByRole("button", {
      name: "Delete recording",
    });

    expect(deleteButton.className).toContain("dark:text-red-400");
    expect(deleteButton.className).toContain("dark:hover:bg-red-950/50");
    expect(deleteButton.className).toContain("dark:hover:text-red-300");
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
    const accessoryRoot = transcriptSlot?.parentElement;
    expect(transcriptCard?.className).not.toContain("min-h-[114px]");
    expect(transcriptCard?.className).not.toContain("min-h-[96px]");
    expect(transcriptSlot?.className).not.toContain("min-h-[114px]");
    expect(transcriptSlot?.className).toContain("shrink-0");
    expect(accessoryRoot?.className).not.toContain("h-full");
    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Regenerate/ }));

    await waitFor(() => {
      expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    });
  });

  it("hides the audio timeline while the transcript panel is collapsed", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio
        hasTranscript
        isTranscriptExpanded={false}
      />,
    );

    expect(screen.queryByTestId("timeline")).toBeNull();
    expect(screen.queryByTestId("transcript")).toBeNull();
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
        hasAudio
        hasTranscript
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
        hasAudio
        hasTranscript
        isTranscriptExpanded={false}
      />,
    );

    expect(screen.getByText("Uploading...")).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Stop transcription" }),
    ).toBeNull();
    expect(screen.queryByText("25%")).toBeNull();
  });

  it("keeps the audio timeline slot height stable when expanded", () => {
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

  it("shows transcript skeletons with simple batch status in the body", () => {
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
    expect(screen.getAllByText("Transcribing...")).toHaveLength(2);
    expect(screen.queryByText("25%")).toBeNull();
    expect(screen.getAllByTestId("spinner")).toHaveLength(2);
    expect(screen.getByTestId("transcript-skeleton")).toBeTruthy();
    expect(screen.queryByTestId("transcript")).toBeNull();
  });

  it("renders compiled insights when the insights tab is active", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio={false}
        hasTranscript
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
