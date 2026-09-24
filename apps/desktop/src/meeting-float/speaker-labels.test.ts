import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  LiveTranscriptSegment,
  RenderTranscriptRequest,
  RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

const mocks = vi.hoisted(() => ({ renderTranscriptSegments: vi.fn() }));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: { renderTranscriptSegments: mocks.renderTranscriptSegments },
}));

import { createFloatingSpeakerLabeler } from "./speaker-labels";

const request: RenderTranscriptRequest = {
  transcripts: [{ started_at: 1_000, words: [], assignments: [] }],
  participant_human_ids: ["remote"],
  self_human_id: "self",
  humans: [
    { human_id: "self", name: "John" },
    { human_id: "remote", name: "Artem" },
  ],
  speaker_context: {
    intervals: [
      {
        start_ms: 1_000,
        end_ms: 60_000,
        active_call: true,
        calendar_call: false,
        mic_isolated: true,
        shared_microphone: false,
        title: "",
        self_names: [],
        participants: [{ human_id: "remote", name: "Artem" }],
      },
    ],
  },
};

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

function rendered(
  live: LiveTranscriptSegment,
  speaker: { label: string; humanId?: string } | null,
  id = live.id,
): RenderedTranscriptSegment {
  return {
    ...live,
    id,
    speaker_label: speaker?.label ?? "Speaker 1",
    provisional_speaker: speaker
      ? {
          name: speaker.label,
          human_id: speaker.humanId ?? null,
          reason: "sole_remote_participant",
        }
      : null,
  };
}

function deferred() {
  let resolve!: (value: unknown) => void;
  const promise = new Promise((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

describe("createFloatingSpeakerLabeler", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("labels each live segment from the native resolver, keyed by segment id", async () => {
    const mic = segment("mic", "DirectMic", 0);
    const remote = segment("remote", "RemoteParty", 2);
    mocks.renderTranscriptSegments.mockResolvedValue({
      status: "ok",
      data: [
        rendered(mic, { label: "John", humanId: "self" }),
        rendered(remote, { label: "Artem", humanId: "remote" }),
      ],
    });
    const onChange = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onChange);

    labeler.update("session", [mic, remote], request);
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledTimes(1));

    expect(labeler.labels.get("mic")).toEqual({
      label: "John",
      humanId: "self",
    });
    expect(labeler.labels.get("remote")).toEqual({
      label: "Artem",
      humanId: "remote",
    });
    expect(mocks.renderTranscriptSegments).toHaveBeenCalledWith(
      expect.objectContaining({
        speaker_context: request.speaker_context,
        preview: [
          expect.objectContaining({ id: "mic", speaker_label: "" }),
          expect.objectContaining({ id: "remote", speaker_label: "" }),
        ],
      }),
    );
  });

  it("shows the latest part's speaker when a context boundary splits a segment", async () => {
    const remote = segment("seg:end:900", "RemoteParty", 0);
    mocks.renderTranscriptSegments.mockResolvedValue({
      status: "ok",
      data: [
        rendered(remote, null, "seg:end:900:0"),
        rendered(
          remote,
          { label: "Artem", humanId: "remote" },
          "seg:end:900:400",
        ),
      ],
    });
    const labeler = createFloatingSpeakerLabeler(vi.fn());

    labeler.update("session", [remote], request);
    await vi.waitFor(() => expect(labeler.labels.size).toBe(1));

    expect(labeler.labels.get("seg:end:900")?.label).toBe("Artem");
  });

  it("numbers unresolved voices once per session so the bounded window cannot renumber them", async () => {
    const a = segment("a", "RemoteParty", 4);
    const b = segment("b", "RemoteParty", 7);
    const c = segment("c", "RemoteParty", 4);
    mocks.renderTranscriptSegments
      .mockResolvedValueOnce({
        status: "ok",
        data: [rendered(a, null), rendered(b, null)],
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: [rendered(b, null), rendered(c, null)],
      });
    const onChange = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onChange);

    labeler.update("session", [a, b], request);
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledTimes(1));
    expect(labeler.labels.get("a")?.label).toBe("Speaker 1");
    expect(labeler.labels.get("b")?.label).toBe("Speaker 2");

    labeler.update("session", [b, c], request);
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledTimes(2));
    expect(labeler.labels.get("b")?.label).toBe("Speaker 2");
    expect(labeler.labels.get("c")?.label).toBe("Speaker 1");
  });

  it("drops a stale answer and starts over when the session changes", async () => {
    const remote = segment("remote", "RemoteParty", 0);
    const first = deferred();
    const second = deferred();
    mocks.renderTranscriptSegments
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const onChange = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onChange);

    labeler.update("session", [remote], request);
    labeler.update("session", [remote], request);
    first.resolve({
      status: "ok",
      data: [rendered(remote, { label: "Stale", humanId: "remote" })],
    });
    second.resolve({
      status: "ok",
      data: [rendered(remote, { label: "Artem", humanId: "remote" })],
    });
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledTimes(1));
    expect(labeler.labels.get("remote")?.label).toBe("Artem");

    labeler.update("other", [], request);
    expect(labeler.labels.size).toBe(0);
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(mocks.renderTranscriptSegments).toHaveBeenCalledTimes(2);
  });

  it("forgets names and drops the in-flight answer once the recording context is gone", async () => {
    const remote = segment("remote", "RemoteParty", 0);
    const pending = deferred();
    mocks.renderTranscriptSegments
      .mockResolvedValueOnce({
        status: "ok",
        data: [rendered(remote, { label: "Artem", humanId: "remote" })],
      })
      .mockReturnValueOnce(pending.promise);
    const onChange = vi.fn();
    const labeler = createFloatingSpeakerLabeler(onChange);

    labeler.update("session", [remote], request);
    await vi.waitFor(() => expect(labeler.labels.size).toBe(1));
    labeler.update("session", [remote], request);
    labeler.update("session", [remote], null);
    expect(labeler.labels.size).toBe(0);
    expect(onChange).toHaveBeenCalledTimes(2);

    pending.resolve({
      status: "ok",
      data: [rendered(remote, { label: "Artem", humanId: "remote" })],
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(labeler.labels.size).toBe(0);
    expect(onChange).toHaveBeenCalledTimes(2);
  });

  it("does not call the resolver without a recording context or live session", () => {
    const labeler = createFloatingSpeakerLabeler(vi.fn());
    labeler.update("session", [segment("mic", "DirectMic", 0)], null);
    labeler.update(null, [segment("mic", "DirectMic", 0)], request);
    expect(mocks.renderTranscriptSegments).not.toHaveBeenCalled();
  });
});
