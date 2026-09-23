import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  LiveTranscriptSegment,
  RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

const mocks = vi.hoisted(() => ({
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: { renderTranscriptSegments: mocks.renderTranscriptSegments },
}));

import { createFloatingSpeakerLabeler } from "./speaker-labels";

import { SegmentKeyUtils } from "~/stt/live-segment";

const request = {
  speaker_context: { intervals: [] },
  transcripts: [],
  participant_human_ids: [],
  self_human_id: "self",
  humans: [],
} satisfies RenderTranscriptRequest;

function segment(
  id: string,
  channel: "DirectMic" | "RemoteParty",
  speaker_index: number,
): LiveTranscriptSegment {
  return {
    id,
    key: { channel, speaker_index, speaker_human_id: null },
    start_ms: 0,
    end_ms: 100,
    text: id,
    words: [],
  };
}

const mic = SegmentKeyUtils.serialize(segment("m", "DirectMic", 0).key);
const remote = SegmentKeyUtils.serialize(segment("r", "RemoteParty", 0).key);

describe("createFloatingSpeakerLabeler", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("keeps anonymous numbers stable and only upgrades them to names", async () => {
    const onLabels = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onLabels);
    const segments = [
      segment("m", "DirectMic", 0),
      segment("r", "RemoteParty", 0),
    ];

    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        { ...segments[0], speaker_label: "Speaker 1" },
        { ...segments[1], speaker_label: "Speaker 2" },
      ],
    });
    labeler.resolve("session", segments, request);
    await vi.waitFor(() => expect(onLabels).toHaveBeenCalledTimes(1));

    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        { ...segments[1], speaker_label: "Speaker 1" },
        { ...segments[0], speaker_label: "John" },
      ],
    });
    labeler.resolve("session", segments, request);
    await vi.waitFor(() => expect(onLabels).toHaveBeenCalledTimes(2));

    expect(labeler.labels.get(mic)).toBe("John");
    expect(labeler.labels.get(remote)).toBe("Speaker 2");
  });

  it("drops labels when the live session changes", async () => {
    const labeler = createFloatingSpeakerLabeler(() => {});
    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [{ ...segment("m", "DirectMic", 0), speaker_label: "John" }],
    });
    labeler.resolve("a", [segment("m", "DirectMic", 0)], request);
    await vi.waitFor(() => expect(labeler.labels.size).toBe(1));

    labeler.resolve("b", [], request);
    expect(labeler.labels.size).toBe(0);
  });
});
