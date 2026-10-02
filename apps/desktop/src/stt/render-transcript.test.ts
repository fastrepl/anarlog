import { beforeEach, describe, expect, it, vi } from "vitest";

const { renderTranscriptSegmentsCommand } = vi.hoisted(() => ({
  renderTranscriptSegmentsCommand: vi.fn(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    renderTranscriptSegments: renderTranscriptSegmentsCommand,
  },
}));

import {
  buildRenderTranscriptRequestFromRows,
  buildUnsplitRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows,
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
  resolveScopedWordHumanIds,
  type TranscriptRow,
} from "./render-transcript";

const transcripts = {
  late: {
    started_at: 5_000,
    words: [
      {
        id: "late-word",
        text: " later",
        start_ms: 100,
        end_ms: 200,
        channel: 1,
      },
    ],
    speaker_hints: [
      {
        word_id: "late-word",
        type: "user_speaker_assignment",
        value: { human_id: "remote" },
      },
    ],
  },
  early: {
    started_at: 1_000,
    words: [
      {
        id: "early-word",
        text: " hello",
        start_ms: 0,
        end_ms: 100,
        channel: 0,
      },
    ],
    speaker_hints: [],
  },
  unordered: {
    started_at: 2_000,
    words: [
      {
        id: "unordered-word",
        text: " hello",
        start_ms: 0,
        end_ms: 100,
        channel: 1,
      },
    ],
    speaker_hints: [
      {
        word_id: "unordered-word",
        type: "user_speaker_assignment",
        value: { human_id: "remote" },
      },
      {
        word_id: "unordered-word",
        type: "provider_speaker_index",
        value: { channel: 1, speaker_index: 2 },
      },
    ],
  },
  segmentOnly: {
    started_at: 2_000,
    words: [
      {
        id: "segment-word-1",
        text: " hello",
        start_ms: 0,
        end_ms: 100,
        channel: 1,
      },
      {
        id: "segment-word-2",
        text: " there",
        start_ms: 100,
        end_ms: 200,
        channel: 1,
      },
    ],
    speaker_hints: [
      {
        word_id: "segment-word-1",
        type: "provider_speaker_index",
        value: { channel: 1, speaker_index: 2 },
      },
      {
        word_id: "segment-word-2",
        type: "provider_speaker_index",
        value: { channel: 1, speaker_index: 2 },
      },
      {
        word_id: "segment-word-1",
        type: "user_speaker_assignment",
        value: {
          human_id: "remote",
          scope: "segment",
          word_ids: ["segment-word-1", "segment-word-2"],
        },
      },
    ],
  },
} as const;

function createRequest(
  transcriptIds: Array<keyof typeof transcripts> = ["late", "early"],
  participantIds = ["self", "remote"],
) {
  return buildRenderTranscriptRequestFromRows(
    transcriptIds.map(
      (transcriptId) => transcripts[transcriptId],
    ) as unknown as TranscriptRow[],
    {
      selfHumanId: "self",
      humans: [
        { human_id: "self", name: "Me" },
        { human_id: "remote", name: "Remote" },
        { human_id: "third", name: "Third" },
      ],
    },
    participantIds,
  );
}

describe("buildRenderTranscriptRequestFromRows", () => {
  beforeEach(() => {
    renderTranscriptSegmentsCommand.mockReset();
  });

  it("passes raw transcript rows and session participant ids to Rust", () => {
    const request = createRequest();

    expect(request).not.toBeNull();
    expect(
      request?.transcripts.map((transcript) => ({
        started_at: transcript.started_at,
        word_ids: transcript.words.map((word) => word.id),
      })),
    ).toEqual([
      {
        started_at: 5_000,
        word_ids: ["late-word"],
      },
      {
        started_at: 1_000,
        word_ids: ["early-word"],
      },
    ]);
    expect(request?.participant_human_ids).toEqual(["self", "remote"]);
    expect(request?.self_human_id).toBe("self");
  });

  it("groups synthetic channel chunks without losing individual word ids", () => {
    const word = (
      id: string,
      text: string,
      start_ms: number,
      channel: number,
      chunk_start_ms: number,
    ) => ({
      id,
      text,
      start_ms,
      end_ms: start_ms + 400,
      channel,
      metadata: {
        timing: { source: "synthetic_text", chunk_start_ms },
      },
    });
    const request = buildRenderTranscriptRequestFromRows([
      {
        started_at: 1_000,
        words: [
          word("mic-1", " Hello", 0, 0, 0),
          word("mic-2", " world.", 400, 0, 0),
          word("remote-1", " Remote", 0, 1, 0),
          word("remote-2", " reply.", 400, 1, 0),
          word("mic-3", " Later.", 29_500, 0, 29_500),
          word("remote-3", " Next.", 29_500, 1, 29_500),
        ],
        speaker_hints: [],
      },
    ]);

    expect(
      request?.transcripts.map((transcript) =>
        transcript.words.map((word) => word.id),
      ),
    ).toEqual([
      ["mic-1", "mic-2"],
      ["remote-1", "remote-2"],
      ["mic-3"],
      ["remote-3"],
    ]);
    expect(request?.transcripts[0]?.words.map((word) => word.text)).toEqual([
      " Hello",
      " world.",
    ]);
  });

  it("keeps identity and refinement inputs whole across synthetic channels", () => {
    const rows = [
      {
        words: [
          {
            id: "mic",
            text: " Mic",
            start_ms: 0,
            end_ms: 400,
            channel: 0,
            metadata: {
              timing: { source: "synthetic_text", chunk_start_ms: 0 },
            },
          },
          {
            id: "remote",
            text: " Remote",
            start_ms: 0,
            end_ms: 400,
            channel: 1,
            metadata: {
              timing: { source: "synthetic_text", chunk_start_ms: 0 },
            },
          },
        ],
        speaker_hints: [
          {
            word_id: "remote",
            type: "user_speaker_assignment",
            value: {
              human_id: "alice",
              scope: "segment",
              word_ids: ["remote"],
            },
          },
        ],
      },
    ];

    expect(
      buildRenderTranscriptRequestFromRows(rows)?.transcripts,
    ).toHaveLength(2);
    const unsplit = buildUnsplitRenderTranscriptRequestFromRows(rows);
    expect(unsplit?.transcripts).toHaveLength(1);
    expect(unsplit?.transcripts[0]?.words.map((word) => word.id)).toEqual([
      "mic",
      "remote",
    ]);
    expect(
      resolveScopedWordHumanIds(unsplit!.transcripts[0]!).get("remote"),
    ).toBe("alice");
  });

  it("keeps genuinely timed channels in one chronological render input", () => {
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          {
            id: "mic",
            text: " Hello",
            start_ms: 0,
            end_ms: 400,
            channel: 0,
            metadata: { timing: { source: "provider_word" } },
          },
          {
            id: "remote",
            text: " Reply",
            start_ms: 200,
            end_ms: 600,
            channel: 1,
            metadata: { timing: { source: "provider_word" } },
          },
        ],
      },
    ]);

    expect(request?.transcripts).toHaveLength(1);
    expect(request?.transcripts[0]?.words.map((word) => word.id)).toEqual([
      "mic",
      "remote",
    ]);
  });

  it("groups an unaligned mic chunk while retaining diarized remote words", () => {
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          {
            id: "mic-1",
            text: " Mic",
            start_ms: 0,
            end_ms: 400,
            channel: 0,
            metadata: {
              timing: { source: "synthetic_text", chunk_start_ms: 0 },
            },
          },
          {
            id: "mic-2",
            text: " words.",
            start_ms: 400,
            end_ms: 800,
            channel: 0,
            metadata: {
              timing: { source: "synthetic_text", chunk_start_ms: 0 },
            },
          },
          {
            id: "remote-1",
            text: " Remote",
            start_ms: 1_000,
            end_ms: 1_500,
            channel: 1,
            metadata: { timing: { source: "provider_segment_interpolated" } },
          },
          {
            id: "remote-2",
            text: " reply.",
            start_ms: 1_500,
            end_ms: 2_000,
            channel: 1,
            metadata: { timing: { source: "provider_segment_interpolated" } },
          },
        ],
        speaker_hints: [
          {
            word_id: "remote-1",
            type: "provider_speaker_index",
            value: { speaker_index: 0 },
          },
          {
            word_id: "remote-2",
            type: "provider_speaker_index",
            value: { speaker_index: 0 },
          },
        ],
      },
    ]);

    expect(
      request?.transcripts.map((transcript) =>
        transcript.words.map((word) => word.id),
      ),
    ).toEqual([
      ["mic-1", "mic-2"],
      ["remote-1", "remote-2"],
    ]);
    expect(
      request?.transcripts[1]?.words.map((word) => word.speaker_index),
    ).toEqual([0, 0]);
  });

  it("passes through all mapped participant ids for Rust-side resolution", () => {
    const request = createRequest(["early"], ["self", "remote", "third"]);

    expect(request?.participant_human_ids).toEqual(["self", "remote", "third"]);
  });

  it("does not enable context mode before recording evidence exists", () => {
    const request = buildRenderTranscriptRequestFromRows(
      [transcripts.early] as unknown as TranscriptRow[],
      {
        selfHumanId: "self",
        humans: [
          { human_id: "self", name: "Me" },
          { human_id: "remote", name: "Marco" },
        ],
      },
      ["remote"],
      { intervals: [] },
    );

    expect(request?.speaker_context).toBeUndefined();
    expect(request?.participant_human_ids).toEqual(["remote"]);
  });

  it("applies provider speaker hints before user assignments regardless of storage order", () => {
    const request = createRequest(["unordered"]);

    expect(request?.transcripts[0]?.words[0]?.speaker_index).toBe(2);
    expect(request?.transcripts[0]?.assignments).toEqual([
      {
        human_id: "remote",
        scope: {
          kind: "channel_speaker",
          channel: "RemoteParty",
          speaker_index: 2,
        },
      },
    ]);
  });

  it("keeps an explicit matching-speaker assignment when its anchor lacks a provider hint", () => {
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          {
            id: "anchor-word",
            text: " hello",
            start_ms: 0,
            end_ms: 100,
            channel: 1,
          },
          {
            id: "hinted-word",
            text: " again",
            start_ms: 100,
            end_ms: 200,
            channel: 1,
          },
        ],
        speaker_hints: [
          {
            word_id: "hinted-word",
            type: "provider_speaker_index",
            value: { channel: 1, speaker_index: 2 },
          },
          {
            word_id: "anchor-word",
            type: "user_speaker_assignment",
            value: {
              human_id: "remote",
              scope: "speaker",
              channel: 1,
              speaker_index: 2,
            },
          },
        ],
      },
    ]);

    expect(request?.transcripts[0]?.assignments).toEqual([
      {
        human_id: "remote",
        scope: {
          kind: "channel_speaker",
          channel: "RemoteParty",
          speaker_index: 2,
        },
      },
    ]);
  });

  it("applies a live speaker assignment before its anchor word is persisted", () => {
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          {
            id: "persisted-word",
            text: " hello",
            start_ms: 0,
            end_ms: 100,
            channel: 1,
          },
        ],
        speaker_hints: [
          {
            word_id: "live-word",
            type: "user_speaker_assignment",
            value: {
              human_id: "remote",
              scope: "speaker",
              channel: 1,
              speaker_index: 2,
            },
          },
        ],
      },
    ]);

    expect(request?.transcripts[0]?.assignments).toEqual([
      {
        human_id: "remote",
        scope: {
          kind: "channel_speaker",
          channel: "RemoteParty",
          speaker_index: 2,
        },
      },
    ]);
  });

  it("turns segment speaker assignments into word-scoped render assignments", () => {
    const request = createRequest(["segmentOnly"]);

    expect(request?.transcripts[0]?.assignments).toEqual([
      {
        human_id: "remote",
        scope: {
          kind: "words",
          word_ids: ["segment-word-1", "segment-word-2"],
        },
      },
    ]);
  });

  it("collects assigned speaker human ids from transcript rows", () => {
    expect(
      collectAssignedHumanIdsFromTranscriptRows([
        {
          speaker_hints: [
            {
              word_id: "word-1",
              type: "user_speaker_assignment",
              value: JSON.stringify({ human_id: "remote" }),
            },
            {
              word_id: "word-2",
              type: "user_speaker_assignment",
              value: { human_id: "third" },
            },
            {
              word_id: "word-2",
              type: "automatic_speaker_assignment",
              value: { human_id: "automatic" },
            },
            {
              word_id: "word-3",
              type: "provider_speaker_index",
              value: JSON.stringify({ speaker_index: 1 }),
            },
          ],
        },
      ]),
    ).toEqual(["remote", "third", "automatic"]);
  });

  it("orders automatic assignments before explicit user assignments", () => {
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          {
            id: "word-1",
            text: " hello",
            start_ms: 0,
            end_ms: 100,
            channel: 1,
          },
        ],
        speaker_hints: [
          {
            word_id: "word-1",
            type: "user_speaker_assignment",
            value: { human_id: "explicit" },
          },
          {
            word_id: "word-1",
            type: "provider_speaker_index",
            value: { channel: 1, speaker_index: 2 },
          },
          {
            word_id: "word-1",
            type: "automatic_speaker_assignment",
            value: { human_id: "automatic" },
          },
        ],
      },
    ]);

    expect(request?.transcripts[0]?.assignments).toEqual([
      {
        human_id: "automatic",
        scope: {
          kind: "channel_speaker",
          channel: "RemoteParty",
          speaker_index: 2,
        },
      },
      {
        human_id: "explicit",
        scope: {
          kind: "channel_speaker",
          channel: "RemoteParty",
          speaker_index: 2,
        },
      },
    ]);
  });

  it("rounds fractional millisecond timings before invoking Rust", async () => {
    renderTranscriptSegmentsCommand.mockResolvedValue({
      status: "ok",
      data: [],
    });

    await renderTranscriptSegments({
      transcripts: [
        {
          started_at: 1_000.6,
          words: [
            {
              id: "word-1",
              text: " hello",
              start_ms: 10.4,
              end_ms: 19.6,
              channel: 0,
              speaker_index: null,
            },
          ],
          assignments: [],
        },
      ],
      participant_human_ids: [],
      self_human_id: null,
      humans: [],
    });

    expect(renderTranscriptSegmentsCommand).toHaveBeenCalledWith({
      transcripts: [
        {
          started_at: 1_001,
          words: [
            {
              id: "word-1",
              text: " hello",
              start_ms: 10,
              end_ms: 20,
              channel: 0,
              speaker_index: null,
            },
          ],
          assignments: [],
        },
      ],
      participant_human_ids: [],
      self_human_id: null,
      humans: [],
    });
  });

  it("reattaches word metadata after Rust renders transcript segments", async () => {
    renderTranscriptSegmentsCommand.mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "segment-1",
          key: {
            channel: "DirectMic",
            speaker_index: null,
            speaker_human_id: null,
          },
          speaker_label: "You",
          start_ms: 10,
          end_ms: 20,
          text: "hello",
          words: [
            {
              id: "word-1",
              text: "hello",
              start_ms: 10,
              end_ms: 20,
              channel: "DirectMic",
              is_final: true,
            },
          ],
        },
      ],
    });

    const segments = await renderTranscriptSegments({
      transcripts: [
        {
          started_at: 1_000,
          words: [
            {
              id: "word-1",
              text: " hello",
              start_ms: 10,
              end_ms: 20,
              channel: 0,
              speaker_index: null,
              metadata: {
                timing: {
                  source: "synthetic_text",
                },
              },
            } as never,
          ],
          assignments: [],
        },
      ],
      participant_human_ids: [],
      self_human_id: null,
      humans: [],
    });

    expect(segments[0]?.words[0]?.metadata).toEqual({
      timing: {
        source: "synthetic_text",
      },
    });
  });

  it("shows legacy synthetic channels contiguously without changing word ids", async () => {
    const rendered = (
      id: string,
      channel: "DirectMic" | "RemoteParty",
      start_ms: number,
    ) => ({
      id: `segment-${id}`,
      key: { channel, speaker_index: null, speaker_human_id: null },
      speaker_label: channel === "DirectMic" ? "Speaker 1" : "Speaker 2",
      start_ms,
      end_ms: start_ms + 400,
      text: id,
      words: [
        {
          id,
          text: id,
          start_ms,
          end_ms: start_ms + 400,
          channel,
          is_final: true,
        },
      ],
    });
    renderTranscriptSegmentsCommand.mockResolvedValue({
      status: "ok",
      data: [
        rendered("mic-1", "DirectMic", 0),
        rendered("remote-1", "RemoteParty", 0),
        rendered("mic-2", "DirectMic", 400),
        rendered("remote-2", "RemoteParty", 400),
        rendered("mic-3", "DirectMic", 29_500),
        rendered("remote-3", "RemoteParty", 29_500),
      ],
    });
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          {
            id: "mic-1",
            text: " One",
            start_ms: 0,
            end_ms: 400,
            channel: 0,
            metadata: { timing: { source: "synthetic_text" } },
          },
          {
            id: "remote-1",
            text: " Two",
            start_ms: 0,
            end_ms: 400,
            channel: 1,
            metadata: { timing: { source: "synthetic_text" } },
          },
          {
            id: "mic-2",
            text: " three.",
            start_ms: 400,
            end_ms: 800,
            channel: 0,
            metadata: { timing: { source: "synthetic_text" } },
          },
          {
            id: "remote-2",
            text: " four.",
            start_ms: 400,
            end_ms: 800,
            channel: 1,
            metadata: { timing: { source: "synthetic_text" } },
          },
        ],
      },
      {
        words: [
          {
            id: "mic-3",
            text: " Later.",
            start_ms: 29_500,
            end_ms: 29_900,
            channel: 0,
            metadata: { timing: { source: "synthetic_text" } },
          },
          {
            id: "remote-3",
            text: " Next.",
            start_ms: 29_500,
            end_ms: 29_900,
            channel: 1,
            metadata: { timing: { source: "synthetic_text" } },
          },
        ],
      },
    ]);

    expect(
      request?.transcripts.map((transcript) =>
        transcript.words.map((word) => word.id),
      ),
    ).toEqual([
      ["mic-1", "mic-2"],
      ["remote-1", "remote-2"],
      ["mic-3"],
      ["remote-3"],
    ]);

    const segments = await renderTranscriptSegments(request!);
    expect(segments.map((segment) => segment.words[0]?.id)).toEqual([
      "mic-1",
      "mic-2",
      "mic-3",
      "remote-1",
      "remote-2",
      "remote-3",
    ]);
    expect(segments.map((segment) => segment.id)).toEqual([
      "segment-mic-1",
      "segment-mic-2",
      "segment-mic-3",
      "segment-remote-1",
      "segment-remote-2",
      "segment-remote-3",
    ]);

    const mixedWords = structuredClone(
      request!.transcripts
        .slice(0, 2)
        .flatMap((transcript) => transcript.words),
    );
    (mixedWords[0] as { metadata?: unknown }).metadata = {
      timing: { source: "provider_word" },
    };
    const mixed = buildRenderTranscriptRequestFromRows([
      { words: mixedWords },
      {
        words: request!.transcripts
          .slice(2)
          .flatMap((transcript) => transcript.words),
      },
    ]);
    const genuinelyTimed = await renderTranscriptSegments(mixed!);
    expect(genuinelyTimed.map((segment) => segment.words[0]?.id)).toEqual([
      "mic-1",
      "remote-1",
      "mic-2",
      "remote-2",
      "mic-3",
      "remote-3",
    ]);
  });

  it("keeps synthetic chunk lines together after Rust globally sorts rendered segments", async () => {
    const rendered = (
      id: string,
      channel: "DirectMic" | "RemoteParty",
      start_ms: number,
    ) => ({
      id: `segment-${id}`,
      key: { channel, speaker_index: null, speaker_human_id: null },
      speaker_label: channel === "DirectMic" ? "Speaker 1" : "Speaker 2",
      start_ms,
      end_ms: start_ms + 400,
      text: id,
      words: [
        {
          id,
          text: id,
          start_ms,
          end_ms: start_ms + 400,
          channel,
          is_final: true,
        },
      ],
    });
    renderTranscriptSegmentsCommand.mockResolvedValue({
      status: "ok",
      data: [
        rendered("mic-1", "DirectMic", 0),
        rendered("remote-1", "RemoteParty", 0),
        rendered("mic-2", "DirectMic", 400),
        rendered("remote-2", "RemoteParty", 400),
        rendered("mic-3", "DirectMic", 29_500),
      ],
    });
    const word = (
      id: string,
      start_ms: number,
      channel: number,
      chunk_start_ms: number,
    ) => ({
      id,
      text: id,
      start_ms,
      end_ms: start_ms + 400,
      channel,
      metadata: { timing: { source: "synthetic_text", chunk_start_ms } },
    });
    const request = buildRenderTranscriptRequestFromRows([
      {
        words: [
          word("mic-1", 0, 0, 0),
          word("remote-1", 0, 1, 0),
          word("mic-2", 400, 0, 0),
          word("remote-2", 400, 1, 0),
          word("mic-3", 29_500, 0, 29_500),
        ],
      },
    ]);

    const segments = await renderTranscriptSegments(request!);
    expect(segments.map((segment) => segment.words[0]?.id)).toEqual([
      "mic-1",
      "mic-2",
      "remote-1",
      "remote-2",
      "mic-3",
    ]);
    expect(segments.map((segment) => segment.id)).toEqual([
      "segment-mic-1",
      "segment-mic-2",
      "segment-remote-1",
      "segment-remote-2",
      "segment-mic-3",
    ]);
  });

  it("orders chunk starts using each transcript row's started-at offset", async () => {
    const rendered = (
      id: string,
      channel: "DirectMic" | "RemoteParty",
      start_ms: number,
    ) => ({
      id: `segment-${id}`,
      key: { channel, speaker_index: null, speaker_human_id: null },
      speaker_label: channel,
      start_ms,
      end_ms: start_ms + 400,
      text: id,
      words: [
        {
          id,
          text: id,
          start_ms,
          end_ms: start_ms + 400,
          channel,
          is_final: true,
        },
      ],
    });
    renderTranscriptSegmentsCommand.mockResolvedValue({
      status: "ok",
      data: [
        rendered("early-mic", "DirectMic", 29_500),
        rendered("early-remote", "RemoteParty", 29_500),
        rendered("later-mic", "DirectMic", 59_000),
        rendered("later-remote", "RemoteParty", 59_000),
      ],
    });
    const word = (id: string, channel: number, chunk_start_ms: number) => ({
      id,
      text: id,
      start_ms: chunk_start_ms,
      end_ms: chunk_start_ms + 400,
      channel,
      metadata: { timing: { source: "synthetic_text", chunk_start_ms } },
    });
    const request = buildRenderTranscriptRequestFromRows([
      {
        started_at: 1_000,
        words: [word("early-mic", 0, 29_500), word("early-remote", 1, 29_500)],
      },
      {
        started_at: 60_000,
        words: [word("later-mic", 0, 0), word("later-remote", 1, 0)],
      },
    ]);

    const segments = await renderTranscriptSegments(request!);
    expect(segments.map((segment) => segment.words[0]?.id)).toEqual([
      "early-mic",
      "early-remote",
      "later-mic",
      "later-remote",
    ]);
  });

  it("groups a legacy synthetic portion beside a genuinely timed row", async () => {
    const rendered = (
      id: string,
      channel: "DirectMic" | "RemoteParty",
      start_ms: number,
    ) => ({
      id: `segment-${id}`,
      key: { channel, speaker_index: null, speaker_human_id: null },
      speaker_label: channel,
      start_ms,
      end_ms: start_ms + 400,
      text: id,
      words: [
        {
          id,
          text: id,
          start_ms,
          end_ms: start_ms + 400,
          channel,
          is_final: true,
        },
      ],
    });
    renderTranscriptSegmentsCommand.mockResolvedValue({
      status: "ok",
      data: [
        rendered("mic-1", "DirectMic", 0),
        rendered("remote-1", "RemoteParty", 0),
        rendered("mic-2", "DirectMic", 29_500),
        rendered("remote-2", "RemoteParty", 29_500),
        rendered("timed", "RemoteParty", 59_000),
      ],
    });
    const legacy = (id: string, start_ms: number, channel: number) => ({
      id,
      text: id,
      start_ms,
      end_ms: start_ms + 400,
      channel,
      metadata: { timing: { source: "synthetic_text" } },
    });
    const request = buildRenderTranscriptRequestFromRows([
      {
        started_at: 1_000,
        words: [
          legacy("mic-1", 0, 0),
          legacy("remote-1", 0, 1),
          legacy("mic-2", 29_500, 0),
          legacy("remote-2", 29_500, 1),
        ],
      },
      {
        started_at: 60_000,
        words: [
          {
            id: "timed",
            text: " Timed",
            start_ms: 0,
            end_ms: 400,
            channel: 1,
            metadata: { timing: { source: "provider_word" } },
          },
        ],
      },
    ]);

    const segments = await renderTranscriptSegments(request!);
    expect(segments.map((segment) => segment.words[0]?.id)).toEqual([
      "mic-1",
      "mic-2",
      "remote-1",
      "remote-2",
      "timed",
    ]);
  });
});

describe("getRenderTranscriptRequestKey", () => {
  it("keeps large transcript payloads out of query keys", () => {
    const request = createRequest();

    expect(getRenderTranscriptRequestKey(request)).toMatch(/^\d+:\d+:\d+:/);
  });

  it.each([
    {
      name: "rendered transcript inputs change",
      change: (request: NonNullable<ReturnType<typeof createRequest>>) => ({
        ...request,
        transcripts: request.transcripts.map((transcript, index) =>
          index === 0
            ? {
                ...transcript,
                words: transcript.words.map((word, wordIndex) =>
                  wordIndex === 0 ? { ...word, text: " changed" } : word,
                ),
              }
            : transcript,
        ),
      }),
    },
    {
      name: "speaker assignments change",
      change: (request: NonNullable<ReturnType<typeof createRequest>>) => ({
        ...request,
        transcripts: request.transcripts.map((transcript, index) =>
          index === 0
            ? {
                ...transcript,
                assignments: [
                  {
                    human_id: "third",
                    scope: {
                      kind: "channel",
                      channel: "RemoteParty",
                    },
                  } as const,
                ],
              }
            : transcript,
        ),
      }),
    },
  ])("changes when $name", ({ change }) => {
    const request = createRequest();

    expect(getRenderTranscriptRequestKey(change(request!))).not.toBe(
      getRenderTranscriptRequestKey(request),
    );
  });
});
