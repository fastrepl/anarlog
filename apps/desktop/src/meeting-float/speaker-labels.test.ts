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

  it("gives evicted-window newcomers a fresh number and applies identity withdrawals", async () => {
    const onLabels = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onLabels);
    const a = segment("a", "RemoteParty", 0);
    const b = segment("b", "RemoteParty", 1);
    const c = segment("c", "RemoteParty", 2);

    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        { ...a, speaker_label: "John" },
        { ...b, speaker_label: "Speaker 1" },
      ],
    });
    labeler.resolve("session", [a, b], request);
    await vi.waitFor(() => expect(onLabels).toHaveBeenCalledTimes(1));

    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        { ...a, speaker_label: "Speaker 1" },
        { ...b, speaker_label: "Speaker 2" },
        { ...c, speaker_label: "Speaker 3" },
      ],
    });
    labeler.resolve("session", [a, b, c], request);
    await vi.waitFor(() => expect(onLabels).toHaveBeenCalledTimes(2));

    expect(
      [a, b, c].map((s) =>
        labeler.labels.get(SegmentKeyUtils.serialize(s.key)),
      ),
    ).toEqual(["Speaker 2", "Speaker 1", "Speaker 3"]);
  });

  it("prefers a resolved name over an anonymous part of the same key and follows the current participant cap", async () => {
    const onLabels = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onLabels);
    const m = segment("m", "DirectMic", 0);
    const r0 = segment("r0", "RemoteParty", 0);
    const r1 = segment("r1", "RemoteParty", 1);
    const r2 = segment("r2", "RemoteParty", 2);
    const twoPeople = { ...request, participant_human_ids: ["self", "a"] };

    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        { ...m, speaker_label: "Speaker 1" },
        { ...m, id: "m2", speaker_label: "John" },
        { ...r0, speaker_label: "Speaker 2" },
        { ...r1, speaker_label: "Speaker 3" },
        { ...r2, speaker_label: "Speaker 4" },
      ],
    });
    labeler.resolve("session", [m, r0, r1, r2], twoPeople);
    await vi.waitFor(() => expect(onLabels).toHaveBeenCalledTimes(1));
    const get = (s: LiveTranscriptSegment) =>
      labeler.labels.get(SegmentKeyUtils.serialize(s.key));
    expect([m, r0, r1, r2].map(get)).toEqual([
      "John",
      "Speaker 1",
      "Speaker 2",
      "Speaker 2",
    ]);

    mocks.renderTranscriptSegments.mockResolvedValueOnce({
      status: "ok",
      data: [
        { ...r0, speaker_label: "Speaker 1" },
        { ...r1, speaker_label: "Speaker 2" },
        { ...r2, speaker_label: "Speaker 3" },
      ],
    });
    labeler.resolve("session", [r0, r1, r2], {
      ...request,
      participant_human_ids: ["self", "a", "b"],
    });
    await vi.waitFor(() => expect(onLabels).toHaveBeenCalledTimes(2));
    expect([r0, r1, r2].map(get)).toEqual([
      "Speaker 1",
      "Speaker 2",
      "Speaker 3",
    ]);
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
