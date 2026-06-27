import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SegmentHeader } from "./segment-header";

import type { Segment } from "~/stt/live-segment";

const { useStoreMock } = vi.hoisted(() => ({
  useStoreMock: vi.fn(),
}));

vi.mock("./speaker-assign", () => ({
  SpeakerAssignPopover: ({ label }: { label: string }) => (
    <button type="button">{label}</button>
  ),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useStore: useStoreMock,
  },
}));

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
  useStoreMock.mockReturnValue(undefined);
});

describe("SegmentHeader", () => {
  it("keeps the speaker label visible without exposing timestamps", () => {
    render(
      <SegmentHeader
        transcriptId="transcript-1"
        segment={
          {
            id: "segment-1",
            key: {
              channel: "RemoteParty",
              speaker_index: 2,
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
              {
                id: "word-2",
                text: "world",
                start_ms: 17_000,
                end_ms: 18_000,
                channel: "RemoteParty",
                is_final: true,
              },
            ],
          } as Segment
        }
      />,
    );

    expect(screen.getByRole("button", { name: "Speaker 3" })).toBeTruthy();
    expect(screen.queryByText("00:12 - 00:18")).toBeNull();
  });
});
