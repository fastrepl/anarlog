import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SegmentHeader } from "./segment-header";

import type { Segment } from "~/stt/live-segment";

const labelState = vi.hoisted(() => ({
  names: {} as Record<string, string>,
  participantIds: [] as string[],
  selfId: undefined as string | undefined,
}));

vi.mock("./speaker-assign", () => ({
  SpeakerAssignPopover: ({ label }: { label: string }) => (
    <button type="button">{label}</button>
  ),
}));

vi.mock("~/stt/queries", () => ({
  useTranscriptLabelContext: () => ({
    getSelfHumanId: () => labelState.selfId,
    getHumanName: (humanId: string) => labelState.names[humanId],
    getParticipantHumanIds: () => labelState.participantIds,
  }),
}));

beforeEach(() => {
  cleanup();
  labelState.names = {};
  labelState.participantIds = [];
  labelState.selfId = undefined;
});

describe("SegmentHeader", () => {
  it("keeps the speaker label visible without exposing timestamps", () => {
    render(
      <SegmentHeader
        transcriptId="transcript-1"
        segment={createRemoteSegment(2)}
      />,
    );

    expect(screen.getByRole("button", { name: "Speaker 3" })).toBeTruthy();
    expect(screen.queryByText("00:12 - 00:18")).toBeNull();
  });

  it("labels remote live segments as the unique other participant", () => {
    labelState.selfId = "self";
    labelState.participantIds = ["self", "remote"];
    labelState.names = { self: "John", remote: "Artem" };

    render(
      <SegmentHeader
        transcriptId="transcript-1"
        segment={createRemoteSegment(0)}
      />,
    );

    expect(screen.getByRole("button", { name: "Artem" })).toBeTruthy();
  });

  it("updates cached remote labels when session participants change", () => {
    labelState.selfId = "self";
    labelState.participantIds = ["self", "remote"];
    labelState.names = { self: "John", remote: "Artem" };
    const segment = createRemoteSegment(0);
    const { rerender } = render(
      <SegmentHeader transcriptId="transcript-1" segment={segment} />,
    );

    expect(screen.getByRole("button", { name: "Artem" })).toBeTruthy();

    labelState.participantIds = ["self", "remote", "remote-2"];
    labelState.names = {
      self: "John",
      remote: "Artem",
      "remote-2": "Taylor",
    };
    rerender(<SegmentHeader transcriptId="transcript-1" segment={segment} />);

    expect(screen.getByRole("button", { name: "Speaker 1" })).toBeTruthy();
  });
});

function createRemoteSegment(speakerIndex: number): Segment {
  return {
    id: "segment-1",
    key: {
      channel: "RemoteParty",
      speaker_index: speakerIndex,
      speaker_human_id: null,
    },
    start_ms: 12_000,
    end_ms: 18_000,
    text: "hello world",
    words: [
      {
        id: "word-1",
        text: "hello",
        start_ms: 12_000,
        end_ms: 13_000,
        channel: "RemoteParty",
        is_final: true,
      },
    ],
  } as Segment;
}
