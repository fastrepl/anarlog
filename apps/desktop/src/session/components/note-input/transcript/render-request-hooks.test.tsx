import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  humanIds: [] as string[],
  humans: [{ human_id: "human-1", name: "Alice" }],
  participantHumanIds: ["human-1"],
  speakerContext: {
    intervals: [] as Array<Record<string, unknown>>,
  },
  transcriptQueryArgs: [] as unknown[],
  transcript: {
    id: "transcript-1",
    ownerUserId: "user-1",
    sessionId: "session-1",
    startedAt: 1000,
    endedAt: 2000,
    words: [
      {
        id: "word-1",
        text: "Hello",
        start_ms: 0,
        end_ms: 500,
        channel: 0,
      },
    ],
    speakerHints: [] as Array<{
      id: string;
      word_id: string;
      type: string;
      value: string;
    }>,
  },
}));

vi.mock("~/stt/queries", () => ({
  getSessionParticipantHumanIds: () =>
    Promise.resolve(mocks.participantHumanIds),
  getSessionTranscriptRecords: () => Promise.resolve([mocks.transcript]),
  getTranscriptHumans: (humanIds: string[]) => {
    mocks.humanIds = humanIds;
    return Promise.resolve(mocks.humans);
  },
  useSessionParticipantHumanIds: () => mocks.participantHumanIds,
  useSessionTranscripts: () => [mocks.transcript],
  useTranscript: (...args: unknown[]) => {
    mocks.transcriptQueryArgs = args;
    return mocks.transcript;
  },
  useTranscriptHumans: (humanIds: string[]) => {
    mocks.humanIds = humanIds;
    return mocks.humans;
  },
}));

vi.mock("~/stt/speaker-context-query", () => ({
  getSpeakerContext: () => Promise.resolve(mocks.speakerContext),
  useSpeakerContext: () => mocks.speakerContext,
}));

import {
  getSessionTranscriptRenderRequest,
  useSessionTranscriptRenderData,
  useTranscriptRenderData,
} from "./render-request-hooks";

describe("SQLite transcript render data", () => {
  beforeEach(() => {
    mocks.humanIds = [];
    mocks.transcriptQueryArgs = [];
    mocks.humans = [{ human_id: "human-1", name: "Alice" }];
    mocks.participantHumanIds = ["human-1"];
    mocks.speakerContext.intervals = [];
    mocks.transcript.speakerHints = [];
  });

  it("can read only the compacted base for an active transcript", () => {
    renderHook(() => useTranscriptRenderData("transcript-1", false));

    expect(mocks.transcriptQueryArgs).toEqual(["transcript-1", false]);
  });

  it("builds a renderer request from one canonical transcript", () => {
    const { result } = renderHook(() =>
      useTranscriptRenderData("transcript-1"),
    );

    expect(result.current.transcriptRows).toEqual([
      {
        transcriptId: "transcript-1",
        row: {
          started_at: 1000,
          words: mocks.transcript.words,
          speaker_hints: [],
        },
      },
    ]);
    expect(result.current.request).toEqual(
      expect.objectContaining({
        self_human_id: "user-1",
        participant_human_ids: ["human-1"],
        humans: [{ human_id: "human-1", name: "Alice" }],
      }),
    );
    expect(result.current.request?.transcripts[0]?.words[0]?.id).toBe("word-1");
    expect(mocks.humanIds).toEqual(["human-1", "user-1"]);
  });

  it("uses the same canonical rows for session-wide export rendering", () => {
    const { result } = renderHook(() =>
      useSessionTranscriptRenderData("session-1"),
    );

    expect(
      result.current.transcriptRows.map((row) => row.transcriptId),
    ).toEqual(["transcript-1"]);
    expect(result.current.request?.transcripts).toHaveLength(1);
  });

  it("builds the same session render request on demand", async () => {
    mocks.transcript.speakerHints.push(
      {
        id: "word-1:provider_speaker_index",
        word_id: "word-1",
        type: "provider_speaker_index",
        value: JSON.stringify({ channel: 0, speaker_index: 0 }),
      },
      {
        id: "word-1:user_speaker_assignment",
        word_id: "word-1",
        type: "user_speaker_assignment",
        value: JSON.stringify({
          human_id: "human-2",
          scope: "speaker",
          channel: 0,
          speaker_index: 0,
        }),
      },
    );
    mocks.humans = [
      { human_id: "human-1", name: "Alice" },
      { human_id: "human-2", name: "Bob" },
      { human_id: "user-1", name: "Me" },
    ];
    mocks.speakerContext.intervals.push({
      start_ms: 0,
      end_ms: 1000,
      active_call: true,
      calendar_call: false,
      mic_isolated: true,
      shared_microphone: false,
      title: "Meeting",
      self_names: ["Me"],
      participants: [{ human_id: "human-1", name: "Alice" }],
    });

    const { result } = renderHook(() =>
      useSessionTranscriptRenderData("session-1"),
    );

    await expect(
      getSessionTranscriptRenderRequest("session-1"),
    ).resolves.toEqual(result.current.request);
    expect(mocks.humanIds).toEqual(["human-1", "human-2", "user-1"]);
  });
});
