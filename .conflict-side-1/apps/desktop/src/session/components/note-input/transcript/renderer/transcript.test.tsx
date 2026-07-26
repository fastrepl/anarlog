import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  IdentityAssignment,
  RenderTranscriptRequest,
} from "@hypr/plugin-transcription";

import { RenderTranscript } from "./transcript";

import type { RenderLabelContext, Segment } from "~/stt/live-segment";

const mocks = vi.hoisted(() => ({
  assignTranscriptSpeaker: vi.fn(),
  useRenderedTranscriptData: vi.fn(),
  useTranscript: vi.fn(),
  useTranscriptLabelContext: vi.fn(),
  useTranscriptOffset: vi.fn(() => 0),
}));

vi.mock("../../search/context", () => ({
  useSearch: () => null,
}));

vi.mock("./data-hooks", () => ({
  useRenderedTranscriptData: mocks.useRenderedTranscriptData,
  useTranscriptOffset: mocks.useTranscriptOffset,
}));

vi.mock("./word-span", () => ({
  WordSpan: ({ displayText }: { displayText: string }) => (
    <span>{displayText}</span>
  ),
}));

vi.mock("@hypr/ui/components/ui/popover", () => ({
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
  useTranscript: mocks.useTranscript,
  useTranscriptLabelContext: mocks.useTranscriptLabelContext,
}));

describe("RenderTranscript", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest(createSegments(500)),
      segments: createSegments(500),
    });
    mocks.useTranscript.mockReturnValue({ sessionId: "session-1" });
    mocks.useTranscriptLabelContext.mockReturnValue(labelContext);
  });

  it("loads transcript metadata once instead of once per rendered segment", () => {
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

    expect(document.querySelectorAll("section")).toHaveLength(500);
    expect(mocks.useRenderedTranscriptData).toHaveBeenCalledOnce();
    expect(mocks.useRenderedTranscriptData).toHaveBeenCalledWith(
      "transcript-1",
      true,
      7,
    );
    expect(mocks.useTranscript).toHaveBeenCalledTimes(1);
    expect(mocks.useTranscript).toHaveBeenCalledWith("transcript-1");
    expect(mocks.useTranscriptLabelContext).toHaveBeenCalledTimes(1);
    expect(mocks.useTranscriptLabelContext).toHaveBeenCalledWith(
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
    mocks.useRenderedTranscriptData.mockReturnValue({
      maxSpeakerNumber: undefined,
      request: createRenderRequest([live], assignments),
      segments: [],
    });
    mocks.useTranscriptLabelContext.mockReturnValue({
      ...labelContext,
      getHumanName: (humanId: string) =>
        humanId === "human-1" ? "Ada" : undefined,
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
});

const labelContext: RenderLabelContext = {
  getSelfHumanId: () => "self",
  getHumanName: () => undefined,
  getParticipantHumanIds: () => [],
};

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
