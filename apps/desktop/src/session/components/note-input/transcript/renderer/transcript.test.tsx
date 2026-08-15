import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  IdentityAssignment,
  RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import { RenderTranscript } from "./transcript";

import type { Segment } from "~/stt/live-segment";

const mocks = vi.hoisted(() => ({
  assignTranscriptSpeaker: vi.fn(),
  search: null as null | {
    activeMatchId: string | null;
    caseSensitive: boolean;
    isVisible: boolean;
    query: string;
    wholeWord: boolean;
  },
  useRenderedTranscriptData: vi.fn(),
  useTranscriptTimelineMetadata: vi.fn(() => ({
    offsetMs: 0,
    sessionId: "session-1",
  })),
}));

vi.mock("../../search/context", () => ({
  useSearch: () => mocks.search,
}));

vi.mock("./data-hooks", () => ({
  useRenderedTranscriptData: mocks.useRenderedTranscriptData,
  useTranscriptTimelineMetadata: mocks.useTranscriptTimelineMetadata,
}));

vi.mock("./word-span", () => ({
  WordSpan: ({ displayText }: { displayText: string }) => (
    <span>{displayText}</span>
  ),
}));

vi.mock("@anlg/ui/components/ui/popover", () => ({
  AppFloatingPanel: () => null,
  Popover: ({ children }: { children: ReactNode }) => <>{children}</>,
  PopoverContent: () => null,
  PopoverTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("~/calendar/queries", () => ({
  useSessionEventParticipants: () => [],
}));

vi.mock("~/contacts/queries", () => ({
  createHuman: vi.fn(),
  useHumans: () => [],
}));

vi.mock("~/session/queries", () => ({
  addSessionParticipant: vi.fn(),
  useSession: () => null,
  useSessionParticipants: () => [],
}));

vi.mock("~/stt/queries", () => ({
  assignTranscriptSpeaker: mocks.assignTranscriptSpeaker,
}));

describe("RenderTranscript", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.search = null;
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(createSegments(500)),
      segments: createSegments(500),
    });
  });

  it("loads one content record and one metadata projection per transcript", () => {
    render(
      <RenderTranscript
        scrollElement={null}
        isLastTranscript
        shouldScrollToEnd={false}
        transcriptId="transcript-1"
        currentActive
        captureGeneration={7}
        liveSegments={createSegments(500)}
        currentMs={0}
        seek={vi.fn()}
        startPlayback={vi.fn()}
        audioExists
      />,
    );

    expect(document.querySelectorAll("section").length).toBeLessThanOrEqual(20);
    expect(
      document
        .querySelector("[data-transcript-virtual-total]")
        ?.getAttribute("data-transcript-virtual-total"),
    ).toBe("500");
    expect(mocks.useRenderedTranscriptData).toHaveBeenCalledOnce();
    expect(mocks.useRenderedTranscriptData).toHaveBeenCalledWith(
      "transcript-1",
      true,
      7,
    );
    expect(mocks.useTranscriptTimelineMetadata).toHaveBeenCalledTimes(1);
    expect(mocks.useTranscriptTimelineMetadata).toHaveBeenCalledWith(
      "transcript-1",
    );
  });

  it("preserves persisted history when a recovered live preview only has the tail", () => {
    const prefix = createSegment("persisted", 0);
    const live = createSegment("live", 1);
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest([prefix, live]),
      segments: [
        {
          ...prefix,
          end_ms: live.end_ms,
          text: `${prefix.text} ${live.text}`,
          words: [...prefix.words, ...live.words],
        },
      ],
    });

    render(
      <RenderTranscript
        scrollElement={null}
        isLastTranscript
        shouldScrollToEnd={false}
        transcriptId="transcript-1"
        currentActive
        liveSegments={[live]}
        currentMs={0}
        seek={vi.fn()}
        startPlayback={vi.fn()}
        audioExists
      />,
    );

    expect(document.querySelectorAll("section")).toHaveLength(2);
    expect(document.body.textContent).toContain("word-persisted");
    expect(document.body.textContent).toContain("word-live");
  });

  it("switches back to the persisted renderer after capture stops", () => {
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(createSegments(1)),
      segments: createSegments(1),
    });

    render(
      <RenderTranscript
        scrollElement={null}
        isLastTranscript
        shouldScrollToEnd={false}
        transcriptId="transcript-1"
        currentActive={false}
        liveSegments={[]}
        currentMs={0}
        seek={vi.fn()}
        startPlayback={vi.fn()}
        audioExists
      />,
    );

    expect(mocks.useRenderedTranscriptData).toHaveBeenCalledWith(
      "transcript-1",
      false,
      0,
    );
    expect(document.querySelectorAll("section")).toHaveLength(1);
  });

  it("uses the volatile persisted fallback before live segments arrive", () => {
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(createSegments(1)),
      segments: createSegments(1),
    });

    render(
      <RenderTranscript
        scrollElement={null}
        isLastTranscript
        shouldScrollToEnd={false}
        transcriptId="transcript-1"
        currentActive
        liveSegments={[]}
        currentMs={0}
        seek={vi.fn()}
        startPlayback={vi.fn()}
        audioExists
      />,
    );

    expect(mocks.useRenderedTranscriptData).toHaveBeenCalledWith(
      "transcript-1",
      true,
      0,
    );
    expect(document.querySelectorAll("section")).toHaveLength(1);
  });

  it("applies current SQLite speaker assignments to visible live segments", () => {
    const live = createSegment("live", 0);
    const assignments: IdentityAssignment[] = [
      {
        human_id: "human-1",
        scope: {
          kind: "channel_speaker",
          channel: "MixedCapture",
          speaker_index: 0,
        },
      },
    ];
    const request = createRenderRequest([live], assignments);
    request.humans = [{ human_id: "human-1", name: "Ada" }];
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request,
      segments: [],
    });

    render(
      <RenderTranscript
        scrollElement={null}
        isLastTranscript
        shouldScrollToEnd={false}
        transcriptId="transcript-1"
        currentActive
        liveSegments={[live]}
        currentMs={0}
        seek={vi.fn()}
        startPlayback={vi.fn()}
        audioExists
      />,
    );

    expect(screen.getByRole("button", { name: "Ada" })).toBeTruthy();
  });

  it("keeps the DOM bounded for a multi-hour transcript fixture", () => {
    const segments = createSegments(10_000);
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(segments),
      segments,
    });

    renderTranscript();

    expect(document.querySelectorAll("section").length).toBeLessThanOrEqual(20);
    expect(
      document
        .querySelector("[data-transcript-virtual-total]")
        ?.getAttribute("data-transcript-virtual-total"),
    ).toBe("10000");
  });

  it("mounts and scrolls to an active off-screen search match", () => {
    const segments = createSegments(1_000);
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(segments),
      segments,
    });
    mocks.search = {
      activeMatchId: "word-999",
      caseSensitive: false,
      isVisible: true,
      query: "word-999",
      wholeWord: false,
    };
    const scrollElement = createScrollElement();

    renderTranscript(scrollElement);

    expect(
      document.querySelector(
        "section[data-transcript-segment-id='segment-999']",
      ),
    ).toBeTruthy();
    expect(scrollElement.scrollTo).toHaveBeenCalled();
  });

  it("pins the active playback segment outside the viewport", () => {
    const segments = createSegments(1_000);
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(segments),
      segments,
    });

    renderTranscript(null, 99_950);

    expect(
      document.querySelector(
        "section[data-transcript-segment-id='segment-999']",
      ),
    ).toBeTruthy();
    expect(document.querySelectorAll("section").length).toBeLessThanOrEqual(21);
  });

  it("preserves visible segment nodes when incremental updates append", () => {
    const initial = createSegments(500);
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(initial),
      segments: initial,
    });
    const rendered = renderTranscript();
    const first = document.querySelector(
      "section[data-transcript-segment-id='segment-0']",
    );
    const updated = [...initial, createSegment("500", 500)];
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(updated),
      segments: updated,
    });

    rendered.rerender(
      <RenderTranscript
        scrollElement={null}
        isLastTranscript
        shouldScrollToEnd={false}
        transcriptId="transcript-1"
        currentActive={false}
        liveSegments={[]}
        currentMs={0}
        seek={vi.fn()}
        startPlayback={vi.fn()}
        audioExists
      />,
    );

    expect(
      document.querySelector("section[data-transcript-segment-id='segment-0']"),
    ).toBe(first);
    expect(document.querySelectorAll("section").length).toBeLessThanOrEqual(20);
  });
});

function renderTranscript(
  scrollElement: HTMLDivElement | null = null,
  currentMs = 0,
) {
  return render(
    <RenderTranscript
      scrollElement={scrollElement}
      isLastTranscript
      shouldScrollToEnd={false}
      transcriptId="transcript-1"
      currentActive={false}
      liveSegments={[]}
      currentMs={currentMs}
      seek={vi.fn()}
      startPlayback={vi.fn()}
      audioExists
    />,
    scrollElement ? { container: scrollElement } : undefined,
  );
}

function createScrollElement() {
  const element = document.createElement("div");
  Object.defineProperties(element, {
    clientHeight: { value: 600 },
    scrollTop: { value: 0, writable: true },
    scrollTo: { value: vi.fn() },
  });
  document.body.append(element);
  return element;
}

function createSegments(count: number): Segment[] {
  return Array.from({ length: count }, (_, index) =>
    createSegment(String(index), index),
  );
}

function createSegment(id: string, index: number): Segment {
  return {
    id: `segment-${id}`,
    key: {
      channel: "MixedCapture",
      speaker_index: index % 2,
      speaker_human_id: null,
    },
    start_ms: index * 100,
    end_ms: index * 100 + 100,
    text: `word ${index}`,
    words: [
      {
        id: `word-${id}`,
        text: `word-${id}`,
        start_ms: index * 100,
        end_ms: index * 100 + 100,
        channel: "MixedCapture",
        is_final: true,
      },
    ],
  };
}

function createRenderRequest(
  segments: Segment[],
  assignments: IdentityAssignment[] = [],
): RenderTranscriptRequest {
  return {
    humans: [],
    participant_human_ids: [],
    self_human_id: null,
    transcripts: [
      {
        assignments,
        started_at: null,
        words: segments.flatMap((segment) =>
          segment.words.flatMap((word) =>
            word.id
              ? [
                  {
                    id: word.id,
                    text: word.text,
                    start_ms: word.start_ms,
                    end_ms: word.end_ms,
                    channel: 2,
                    speaker_index: segment.key.speaker_index,
                  },
                ]
              : [],
          ),
        ),
      },
    ],
  };
}
