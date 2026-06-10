import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DuringSessionAccessory } from "./during-session";

import type { Segment } from "~/stt/live-segment";

const {
  useCellMock,
  useListenerMock,
  useQueriesMock,
  useQueryMock,
  useRowIdsMock,
  useSliceRowIdsMock,
  useStoreMock,
  useTableMock,
  useValueMock,
} = vi.hoisted(() => ({
  useCellMock: vi.fn(),
  useListenerMock: vi.fn(),
  useQueriesMock: vi.fn(),
  useQueryMock: vi.fn(),
  useRowIdsMock: vi.fn(),
  useSliceRowIdsMock: vi.fn(),
  useStoreMock: vi.fn(),
  useTableMock: vi.fn(),
  useValueMock: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: useQueryMock,
}));

vi.mock("@hypr/ui/components/ui/popover", () => ({
  AppFloatingPanel: ({
    children,
    className,
  }: {
    children: ReactNode;
    className?: string;
  }) => <div className={className}>{children}</div>,
  Popover: ({ children }: { children: ReactNode }) => <>{children}</>,
  PopoverContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  PopoverTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    sessionParticipantsBySession: "sessionParticipantsBySession",
    transcriptBySession: "transcriptBySession",
  },
  QUERIES: {
    sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  },
  UI: {
    useCell: useCellMock,
    useQueries: useQueriesMock,
    useRowIds: useRowIdsMock,
    useSliceRowIds: useSliceRowIdsMock,
    useStore: useStoreMock,
    useTable: useTableMock,
    useValue: useValueMock,
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

describe("DuringSessionAccessory", () => {
  let liveSegments: Segment[];
  let scrollHeight: number;

  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    liveSegments = [segment("All right, let's see.", 0)];
    scrollHeight = 480;

    useQueryMock.mockReturnValue({ data: [] });
    useCellMock.mockReturnValue(undefined);
    useQueriesMock.mockReturnValue(null);
    useRowIdsMock.mockReturnValue([]);
    useSliceRowIdsMock.mockReturnValue([]);
    useStoreMock.mockReturnValue(null);
    useTableMock.mockReturnValue({});
    useValueMock.mockReturnValue(undefined);
    useListenerMock.mockImplementation((selector) =>
      selector({
        live: {
          requestedLiveTranscription: true,
          liveTranscriptionActive: true,
        },
        liveSegments,
      }),
    );

    Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
      configurable: true,
      get: () => scrollHeight,
    });
  });

  it("does not render a footer while recording for batch transcription", () => {
    useListenerMock.mockImplementation((selector) =>
      selector({
        live: {
          requestedLiveTranscription: false,
          liveTranscriptionActive: false,
        },
        liveSegments,
      }),
    );

    const view = render(<DuringSessionAccessory sessionId="session-1" />);

    expect(view.container.firstChild).toBeNull();
  });

  it("uses matching collapsed live transcript vertical padding", () => {
    render(<DuringSessionAccessory sessionId="session-1" />);

    const message = screen.getByText("All right, let's see.");
    const row = message.parentElement?.parentElement;
    expect(row?.className).toContain("py-2");
  });

  it("lets manual scrolling override expanded live transcript bottom pinning", () => {
    const view = render(
      <DuringSessionAccessory sessionId="session-1" isExpanded />,
    );

    const viewport = getLiveTranscriptScrollViewport();
    expect(viewport.scrollTop).toBe(480);

    viewport.scrollTop = 10;
    fireEvent.scroll(viewport);
    expect(viewport.scrollTop).toBe(10);

    scrollHeight = 640;
    liveSegments = [
      liveSegments[0]!,
      segment("I'm going to leave that alone.", 500),
    ];
    view.rerender(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    expect(screen.getByText("I'm going to leave that alone.")).toBeTruthy();
    expect(getLiveTranscriptScrollViewport().scrollTop).toBe(10);
  });

  it("resumes expanded live transcript bottom pinning after scrolling back down", () => {
    const view = render(
      <DuringSessionAccessory sessionId="session-1" isExpanded />,
    );

    const viewport = getLiveTranscriptScrollViewport();
    expect(viewport.scrollTop).toBe(480);

    viewport.scrollTop = 10;
    fireEvent.scroll(viewport);

    scrollHeight = 640;
    liveSegments = [
      liveSegments[0]!,
      segment("I'm going to leave that alone.", 500),
    ];
    view.rerender(<DuringSessionAccessory sessionId="session-1" isExpanded />);
    expect(getLiveTranscriptScrollViewport().scrollTop).toBe(10);

    viewport.scrollTop = 640;
    fireEvent.scroll(viewport);

    scrollHeight = 800;
    liveSegments = [
      liveSegments[0]!,
      liveSegments[1]!,
      segment("Pin new words after I return to the bottom.", 1000),
    ];
    view.rerender(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    expect(
      screen.getByText("Pin new words after I return to the bottom."),
    ).toBeTruthy();
    expect(getLiveTranscriptScrollViewport().scrollTop).toBe(800);
  });

  it("keeps rendered transcript history visible while resumed live segments stream", () => {
    useQueryMock.mockReturnValue({
      data: [segment("Earlier saved transcript.", 0)],
    });
    liveSegments = [segment("New live words.", 500)];

    render(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    expect(screen.getByText("Earlier saved transcript.")).toBeTruthy();
    expect(screen.getByText("New live words.")).toBeTruthy();
  });

  it("does not duplicate rendered segments already represented by live segments", () => {
    const sharedSegment = segment("Shared live words.", 0);
    useQueryMock.mockReturnValue({
      data: [sharedSegment],
    });
    liveSegments = [sharedSegment];

    render(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    expect(screen.getAllByText("Shared live words.")).toHaveLength(1);
  });

  it("truncates long speaker labels inside the chip", () => {
    const label = "Alexandria Catherine Montgomery";

    useStoreMock.mockReturnValue({
      getValue: vi.fn(() => undefined),
      getRow: vi.fn((tableId: string, rowId: string) =>
        tableId === "humans" && rowId === "human-1" ? { name: label } : {},
      ),
    });
    liveSegments = [
      segment("This label should not resize the transcript row.", 0, {
        channel: "RemoteParty",
        speaker_human_id: "human-1",
      }),
    ];

    render(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    const chip = screen.getByTitle(label);
    expect(chip.className).toContain("max-w-full");
    expect(chip.className).toContain("min-w-0");
    expect(screen.getByText(label).className).toContain("truncate");
  });

  it("keeps speaker labels sticky while expanded transcript content scrolls", () => {
    const label = "Artem";

    useStoreMock.mockReturnValue({
      getValue: vi.fn(() => undefined),
      getRow: vi.fn((tableId: string, rowId: string) =>
        tableId === "humans" && rowId === "human-1" ? { name: label } : {},
      ),
    });
    liveSegments = [
      segment("This speaker label should remain visible while scrolling.", 0, {
        channel: "RemoteParty",
        speaker_human_id: "human-1",
      }),
    ];

    render(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    const chip = screen.getByTitle(label);
    expect(chip.className).toContain("sticky");
    expect(chip.className).toContain("top-2.5");
  });

  it("assigns live transcript speaker labels to session participants", () => {
    const setCell = vi.fn();
    const store = {
      forEachRow: vi.fn(
        (tableId: string, callback: (rowId: string) => void) => {
          if (tableId === "humans") {
            callback("human-1");
          }
          if (tableId === "mapping_session_participant") {
            callback("mapping-1");
          }
        },
      ),
      getCell: vi.fn((tableId: string, rowId: string, cellId: string) => {
        if (tableId === "transcripts" && rowId === "transcript-1") {
          if (cellId === "session_id") return "session-1";
          if (cellId === "started_at") return 0;
          if (cellId === "words") {
            return JSON.stringify([
              {
                id: "word-0",
                text: "Remote words.",
                start_ms: 0,
                end_ms: 300,
                channel: 1,
              },
            ]);
          }
          if (cellId === "speaker_hints") return "[]";
        }
        if (tableId === "mapping_session_participant") {
          if (cellId === "session_id") return "session-1";
          if (cellId === "human_id") return "human-1";
        }
        return undefined;
      }),
      getRow: vi.fn((tableId: string, rowId: string) =>
        tableId === "humans" && rowId === "human-1"
          ? { name: "Alex", email: "alex@example.com" }
          : {},
      ),
      getValue: vi.fn(() => "user-1"),
      setCell,
      setRow: vi.fn(),
    };

    useStoreMock.mockReturnValue(store);
    useCellMock.mockReturnValue("session-1");
    useQueriesMock.mockReturnValue({
      getResultRow: vi.fn(() => ({
        human_id: "human-1",
        human_name: "Alex",
        human_email: "alex@example.com",
      })),
    });
    useRowIdsMock.mockReturnValue(["human-1"]);
    useSliceRowIdsMock.mockImplementation((indexId: string) =>
      indexId === "transcriptBySession" ? ["transcript-1"] : ["mapping-1"],
    );
    useValueMock.mockReturnValue("user-1");
    liveSegments = [
      segment("Remote words.", 0, {
        channel: "RemoteParty",
        speaker_index: 0,
      }),
    ];

    render(<DuringSessionAccessory sessionId="session-1" isExpanded />);

    expect(screen.getByRole("button", { name: "Speaker 1" })).toBeTruthy();
    fireEvent.click(screen.getByText("Alex"));

    expect(setCell).toHaveBeenCalledWith(
      "transcripts",
      "transcript-1",
      "speaker_hints",
      expect.any(String),
    );
    const hints = JSON.parse(setCell.mock.calls[0]?.[3] as string);
    expect(hints).toEqual([
      {
        id: "word-0:user_speaker_assignment",
        word_id: "word-0",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "human-1" }),
      },
    ]);
  });
});

function getLiveTranscriptScrollViewport() {
  const viewport = document.querySelector<HTMLDivElement>(
    "[data-live-transcript-scroll]",
  );
  expect(viewport).not.toBeNull();
  return viewport!;
}

function segment(
  text: string,
  startMs: number,
  key: Partial<Segment["key"]> = {},
): Segment {
  return {
    key: {
      channel: "DirectMic",
      speaker_index: null,
      speaker_human_id: null,
      ...key,
    },
    start_ms: startMs,
    end_ms: startMs + 300,
    words: [
      {
        id: `word-${startMs}`,
        text,
        start_ms: startMs,
        end_ms: startMs + 300,
      },
    ],
  } as Segment;
}
