import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LiveTranscriptDelta } from "@hypr/plugin-transcription";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  humanRows: [] as Array<Record<string, unknown>>,
  participantRows: [] as Array<Record<string, unknown>>,
  queryOptions: [] as Array<{
    sql: string;
    params?: unknown[];
    enabled?: boolean;
  }>,
  transcriptRows: [] as Array<Record<string, unknown>>,
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: (options: {
    sql: string;
    params?: unknown[];
    enabled?: boolean;
    mapRows?: (rows: Array<Record<string, unknown>>) => unknown;
  }) => {
    mocks.queryOptions.push(options);
    const rows = options.sql.includes("FROM session_participants")
      ? mocks.participantRows
      : options.sql.includes("FROM humans")
        ? mocks.humanRows
        : mocks.transcriptRows;

    return {
      data:
        options.enabled === false
          ? undefined
          : options.mapRows
            ? options.mapRows(rows)
            : rows,
    };
  },
}));

import {
  applyLiveTranscriptDeltaToDatabase,
  appendTranscriptWordsAndHints,
  assignTranscriptSpeaker,
  createLiveTranscript,
  createTranscript,
  removeHumanSpeakerAssignments,
  useSessionParticipantHumanIds,
  useSessionTranscripts,
  useTranscript,
  useTranscriptHumans,
  useTranscriptLabelContext,
} from "./queries";

describe("transcript SQLite queries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.humanRows = [];
    mocks.participantRows = [];
    mocks.queryOptions = [];
    mocks.transcriptRows = [];
  });

  it("maps canonical transcript JSON into renderer records", () => {
    mocks.transcriptRows = [
      {
        id: "transcript-1",
        owner_user_id: "user-1",
        session_id: "session-1",
        started_at_ms: 1000,
        ended_at_ms: 2000,
        words_json: JSON.stringify([
          {
            id: "word-1",
            text: "Hello",
            start_ms: 0,
            end_ms: 500,
            channel: 0,
          },
        ]),
        speaker_hints_json: JSON.stringify([
          { word_id: "word-1", type: "provider_speaker_index", value: 0 },
        ]),
      },
    ];

    const { result } = renderHook(() => useSessionTranscripts("session-1"));

    expect(result.current).toEqual([
      expect.objectContaining({
        id: "transcript-1",
        ownerUserId: "user-1",
        sessionId: "session-1",
        startedAt: 1000,
        endedAt: 2000,
        words: [expect.objectContaining({ id: "word-1" })],
        speakerHints: [expect.objectContaining({ word_id: "word-1" })],
      }),
    ]);
    expect(mocks.queryOptions[0]?.sql).toContain(
      "ORDER BY started_at_ms, created_at, id",
    );
  });

  it("treats non-array transcript payloads as empty without hiding the row", () => {
    mocks.transcriptRows = [
      {
        id: "transcript-1",
        owner_user_id: "user-1",
        session_id: "session-1",
        started_at_ms: 1000,
        ended_at_ms: null,
        words_json: "{}",
        speaker_hints_json: "null",
      },
    ];

    const { result } = renderHook(() => useTranscript("transcript-1"));

    expect(result.current).toEqual(
      expect.objectContaining({
        id: "transcript-1",
        endedAt: undefined,
        words: [],
        speakerHints: [],
      }),
    );
  });

  it("reads distinct participant human ids", () => {
    mocks.participantRows = [{ human_id: "human-1" }, { human_id: "human-2" }];

    const { result } = renderHook(() =>
      useSessionParticipantHumanIds("session-1"),
    );

    expect(result.current).toEqual(["human-1", "human-2"]);
    expect(mocks.queryOptions[0]?.sql).toContain("deleted_at IS NULL");
  });

  it("deduplicates and sorts ids before loading named humans", () => {
    mocks.humanRows = [
      { id: "human-1", name: "Alice" },
      { id: "human-2", name: "Bob" },
    ];

    const { result } = renderHook(() =>
      useTranscriptHumans(["human-2", "human-1", "human-2", ""]),
    );

    expect(result.current).toEqual([
      { human_id: "human-1", name: "Alice" },
      { human_id: "human-2", name: "Bob" },
    ]);
    expect(mocks.queryOptions[0]?.params).toEqual(["human-1", "human-2"]);
  });

  it("builds speaker labels from canonical owner, participant, and human rows", () => {
    mocks.transcriptRows = [
      {
        id: "transcript-1",
        owner_user_id: "self",
        session_id: "session-1",
        started_at_ms: 1000,
        ended_at_ms: null,
        words_json: "[]",
        speaker_hints_json: "[]",
      },
    ];
    mocks.participantRows = [{ human_id: "self" }, { human_id: "human-1" }];
    mocks.humanRows = [
      { id: "self", name: "John" },
      { id: "human-1", name: "Alice" },
    ];

    const { result } = renderHook(() =>
      useTranscriptLabelContext("transcript-1"),
    );

    expect(result.current?.getSelfHumanId()).toBe("self");
    expect(result.current?.getHumanName("human-1")).toBe("Alice");
    expect(result.current?.getParticipantHumanIds?.()).toEqual([
      "self",
      "human-1",
    ]);
  });

  it("creates the first live transcript delta in one insert", async () => {
    await createLiveTranscript(
      {
        id: "transcript-1",
        sessionId: "session-1",
        ownerUserId: "user-1",
        createdAt: "2026-07-10T12:00:00.000Z",
        startedAt: 1000,
        source: "live_capture",
        provider: "soniox",
        model: "stt-rt-v3",
      },
      liveDelta([
        {
          id: "word-1",
          text: "Hello",
          start_ms: 0,
          end_ms: 500,
          channel: 0,
          state: "final",
          speaker_index: 1,
        },
      ]),
    );

    const statements = mocks.executeTransaction.mock.calls[0]?.[0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(1);
    expect(statements[0]?.sql).toContain("INSERT INTO transcripts");
    expect(statements[0]?.sql).toContain("session.workspace_id");
    expect(statements[0]?.params.slice(0, 8)).toEqual([
      "transcript-1",
      "user-1",
      "live_capture",
      "soniox",
      "stt-rt-v3",
      "",
      1000,
      null,
    ]);
    expect(JSON.parse(String(statements[0]?.params[9]))).toEqual([
      expect.objectContaining({ id: "word-1", text: "Hello" }),
    ]);
    expect(JSON.parse(String(statements[0]?.params[10]))).toEqual([
      expect.objectContaining({
        word_id: "word-1",
        type: "provider_speaker_index",
      }),
    ]);
    const params = statements[0]?.params ?? [];
    expect(params[params.length - 1]).toBe("session-1");
  });

  it("tombstones old session transcripts in the same replacement transaction", async () => {
    await createTranscript({
      id: "transcript-new",
      sessionId: "session-1",
      ownerUserId: "user-1",
      createdAt: "2026-07-10T12:00:00.000Z",
      startedAt: 1000,
      replaceSession: true,
    });

    const statements = mocks.executeTransaction.mock.calls[0]?.[0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(2);
    expect(statements[0]?.sql).toContain("UPDATE transcripts");
    expect(statements[0]?.sql).toContain("deleted_at IS NULL");
    expect(statements[1]?.sql).toContain("INSERT INTO transcripts");
  });

  it("retries a live delta against the latest row after a concurrent write", async () => {
    mocks.execute
      .mockResolvedValueOnce([{ words_json: "[]", speaker_hints_json: "[]" }])
      .mockResolvedValueOnce([
        {
          words_json: JSON.stringify([
            {
              id: "external-word",
              text: "External",
              start_ms: 0,
              end_ms: 100,
              channel: 0,
            },
          ]),
          speaker_hints_json: "[]",
        },
      ]);
    mocks.executeTransaction
      .mockResolvedValueOnce([0])
      .mockResolvedValueOnce([1]);

    await applyLiveTranscriptDeltaToDatabase(
      "transcript-1",
      liveDelta([
        {
          id: "word-2",
          text: "Hello",
          start_ms: 200,
          end_ms: 500,
          channel: 0,
          state: "final",
        },
      ]),
    );

    const retryStatement = mocks.executeTransaction.mock.calls[1]?.[0]?.[0] as {
      params: unknown[];
    };
    expect(JSON.parse(String(retryStatement.params[0]))).toEqual([
      expect.objectContaining({ id: "external-word" }),
      expect.objectContaining({ id: "word-2" }),
    ]);
  });

  it("refuses to overwrite malformed transcript JSON", async () => {
    mocks.execute.mockResolvedValueOnce([
      { words_json: "not-json", speaker_hints_json: "[]" },
    ]);

    await expect(
      appendTranscriptWordsAndHints("transcript-1", [], []),
    ).rejects.toThrow("invalid words data");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("persists speaker assignments through the optimistic transcript update", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        words_json: JSON.stringify([
          {
            id: "word-1",
            text: "Hello",
            start_ms: 0,
            end_ms: 500,
            channel: 1,
          },
        ]),
        speaker_hints_json: "[]",
      },
    ]);

    await assignTranscriptSpeaker({
      transcriptId: "transcript-1",
      segmentKey: {
        channel: "RemoteParty",
        speaker_index: 0,
        speaker_human_id: null,
      },
      humanId: "human-1",
      anchorWordId: "word-1",
      mode: "all",
      wordIds: ["word-1"],
    });

    const statement = mocks.executeTransaction.mock.calls[0]?.[0]?.[0];
    expect(JSON.parse(String(statement?.params[1]))).toEqual([
      expect.objectContaining({
        word_id: "word-1",
        type: "user_speaker_assignment",
      }),
    ]);
  });

  it("removes one human's assignments from every session transcript", async () => {
    mocks.execute
      .mockResolvedValueOnce([{ id: "transcript-1" }])
      .mockResolvedValueOnce([
        {
          words_json: "[]",
          speaker_hints_json: JSON.stringify([
            {
              id: "assignment-1",
              word_id: "word-1",
              type: "user_speaker_assignment",
              value: JSON.stringify({ human_id: "human-1" }),
            },
            {
              id: "assignment-2",
              word_id: "word-2",
              type: "user_speaker_assignment",
              value: JSON.stringify({ human_id: "human-2" }),
            },
          ]),
        },
      ]);

    await removeHumanSpeakerAssignments("session-1", "human-1");

    const statement = mocks.executeTransaction.mock.calls[0]?.[0]?.[0];
    expect(JSON.parse(String(statement?.params[1]))).toEqual([
      expect.objectContaining({ id: "assignment-2" }),
    ]);
  });
});

function liveDelta(
  newWords: LiveTranscriptDelta["new_words"],
  replacedIds: string[] = [],
): LiveTranscriptDelta {
  return { new_words: newWords, replaced_ids: replacedIds, partials: [] };
}
