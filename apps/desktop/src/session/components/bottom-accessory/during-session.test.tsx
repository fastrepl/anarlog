import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DuringSessionAccessory } from "./during-session";

import type { Segment } from "~/stt/live-segment";

const { useListenerMock, useQueryMock, useStoreMock } = vi.hoisted(() => ({
  useListenerMock: vi.fn(),
  useQueryMock: vi.fn(),
  useStoreMock: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: useQueryMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    transcriptBySession: "transcriptBySession",
  },
  UI: {
    useSliceRowIds: vi.fn(() => []),
    useStore: useStoreMock,
    useTable: vi.fn(() => ({})),
    useValue: vi.fn(() => undefined),
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
    useStoreMock.mockReturnValue(null);
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
