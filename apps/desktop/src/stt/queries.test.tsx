import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LiveTranscriptDelta } from "@anlg/plugin-transcription";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve(_statements.map(() => 1)),
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
  flushLiveTranscriptDeltasToDatabase,
  removeHumanSpeakerAssignments,
  updateTranscriptSegmentText,
  useSessionParticipantHumanIds,
  useSessionTranscriptMetadata,
  useSessionTranscripts,
  useTranscript,
  useTranscriptHumans,
  useTranscriptMetadata,
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
      "ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id",
    );
  });

  it("materializes ordered live journal chunks on read", () => {
    mocks.transcriptRows = [
      {
        id: "transcript-1",
        owner_user_id: "user-1",
        session_id: "session-1",
        started_at_ms: 1000,
        ended_at_ms: null,
        words_json: JSON.stringify([
          {
            id: "word-old",
            text: "Old",
            start_ms: 0,
            end_ms: 100,
            channel: 0,
          },
        ]),
        speaker_hints_json: "[]",
        content_revision: 0,
        pending_deltas_json: JSON.stringify([
          liveDelta(
            [
              {
                id: "word-final",
                text: "Final",
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                state: "final",
              },
            ],
            ["word-old"],
          ),
          liveDelta([
            {
              id: "word-next",
              text: "Next",
              start_ms: 100,
              end_ms: 200,
              channel: 0,
              state: "final",
            },
          ]),
        ]),
      },
    ];

    const { result } = renderHook(() => useTranscript("transcript-1"));

    expect(result.current?.words.map((word) => word.id)).toEqual([
      "word-final",
      "word-next",
    ]);
  });

  it("skips live journal replay for an active renderer baseline", () => {
    mocks.transcriptRows = [
      {
        id: "transcript-1",
        owner_user_id: "user-1",
        session_id: "session-1",
        started_at_ms: 1000,
        ended_at_ms: null,
        words_json: JSON.stringify([
          {
            id: "word-base",
            text: "Base",
            start_ms: 0,
            end_ms: 100,
            channel: 0,
          },
        ]),
        speaker_hints_json: "[]",
        content_revision: 0,
        pending_deltas_json: "[]",
      },
    ];

    const { result } = renderHook(() => useTranscript("transcript-1", false));

    expect(result.current?.words.map((word) => word.id)).toEqual(["word-base"]);
    expect(mocks.queryOptions[0]?.sql).not.toContain(
      "FROM transcript_live_deltas AS delta",
    );
  });

  it("projects transcript metadata without transferring content blobs", () => {
    mocks.transcriptRows = [
      {
        id: "transcript-1",
        session_id: "session-1",
        started_at_ms: 1000,
        ended_at_ms: 2000,
        has_words: 1,
      },
    ];

    const session = renderHook(() => useSessionTranscriptMetadata("session-1"));
    const transcript = renderHook(() => useTranscriptMetadata("transcript-1"));

    expect(session.result.current).toEqual([
      {
        id: "transcript-1",
        sessionId: "session-1",
        startedAt: 1000,
        endedAt: 2000,
        hasWords: true,
      },
    ]);
    expect(transcript.result.current).toEqual(session.result.current[0]);
    for (const query of mocks.queryOptions) {
      expect(query.sql).toContain("END AS has_words");
      expect(query.sql).not.toContain("transcript.speaker_hints_json");
      expect(query.sql).not.toMatch(/transcript\.words_json,\s/);
      expect(query.sql).not.toContain(
        "json_array_length(transcript.words_json)",
      );
    }
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
    expect(statements[0]?.sql).toContain(
      "COALESCE(NULLIF(?, ''), session.owner_user_id)",
    );
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

  it("appends a partial capture without tombstoning earlier transcripts", async () => {
    await createTranscript({
      id: "transcript-new",
      sessionId: "session-1",
      ownerUserId: "user-1",
      createdAt: "2026-07-10T12:00:00.000Z",
      startedAt: 1000,
      replaceSession: false,
    });

    const statements = mocks.executeTransaction.mock.calls[0]?.[0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(1);
    expect(statements[0]?.sql).toContain("INSERT INTO transcripts");
    expect(statements[0]?.sql).not.toContain("UPDATE transcripts");
  });

  it("atomically replaces the live capture without a visible duplicate", async () => {
    await createTranscript({
      id: "transcript-new",
      sessionId: "session-1",
      ownerUserId: "user-1",
      createdAt: "2026-07-10T12:00:00.000Z",
      startedAt: 1000,
      replaceTranscriptId: "transcript-current-live",
    });

    const statements = mocks.executeTransaction.mock.calls[0]?.[0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(mocks.executeTransaction).toHaveBeenCalledOnce();
    expect(statements).toHaveLength(2);
    expect(statements[0]?.sql).toContain("UPDATE transcripts");
    expect(statements[0]?.sql).toContain(
      "WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
    );
    expect(statements[0]?.params.slice(-2)).toEqual([
      "transcript-current-live",
      "session-1",
    ]);
    expect(statements[1]?.sql).toContain("INSERT INTO transcripts");
  });

  it("appends a live delta without reading or binding canonical blobs", async () => {
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

    expect(mocks.execute).not.toHaveBeenCalled();
    const statements = mocks.executeTransaction.mock.calls[0]?.[0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(3);
    expect(statements[1]?.sql).toContain("INSERT INTO transcript_live_deltas");
    expect(
      statements.map((statement) => statement.sql).join("\n"),
    ).not.toContain("words_json");
    expect(JSON.parse(String(statements[1]?.params[1]))).toEqual(
      expect.objectContaining({
        new_words: [expect.objectContaining({ id: "word-2" })],
      }),
    );
  });

  it("retries a canonical edit with a small revision token", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          words_json: "[]",
          speaker_hints_json: "[]",
          content_revision: 3,
          pending_deltas_json: "[]",
        },
      ])
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
          content_revision: 4,
          pending_deltas_json: "[]",
        },
      ]);
    mocks.executeTransaction
      .mockResolvedValueOnce([0, 0])
      .mockResolvedValueOnce([1, 0]);

    await appendTranscriptWordsAndHints(
      "transcript-1",
      [
        {
          id: "word-2",
          text: "Hello",
          start_ms: 200,
          end_ms: 500,
          channel: 0,
        },
      ],
      [],
    );

    const retryStatement = mocks.executeTransaction.mock.calls[1]?.[0]?.[0] as {
      sql: string;
      params: unknown[];
    };
    expect(retryStatement.sql).toContain("content_revision = ?");
    expect(retryStatement.sql).not.toContain(
      "words_json = ?\n              AND",
    );
    expect(retryStatement.params[retryStatement.params.length - 1]).toBe(4);
    expect(JSON.parse(String(retryStatement.params[0]))).toEqual([
      expect.objectContaining({ id: "external-word" }),
      expect.objectContaining({ id: "word-2" }),
    ]);
  });

  it("compacts pending chunks deterministically and clears the journal", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        words_json: JSON.stringify([
          {
            id: "word-old",
            text: "Old",
            start_ms: 0,
            end_ms: 100,
            channel: 0,
          },
        ]),
        speaker_hints_json: "[]",
        content_revision: 7,
        pending_deltas_json: JSON.stringify([
          liveDelta(
            [
              {
                id: "word-final",
                text: "Final",
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                state: "final",
              },
            ],
            ["word-old"],
          ),
        ]),
      },
    ]);

    await flushLiveTranscriptDeltasToDatabase("transcript-1");

    const statements = mocks.executeTransaction.mock.calls[0]?.[0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(JSON.parse(String(statements[0]?.params[0]))).toEqual([
      expect.objectContaining({ id: "word-final", text: "Final" }),
    ]);
    const updateParams = statements[0]?.params ?? [];
    expect(updateParams[updateParams.length - 1]).toBe(7);
    expect(statements[1]?.sql).toContain("changes() = 1");
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

  it("updates editable segment text without discarding word identity or timing", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        words_json: JSON.stringify([
          {
            id: "word-1",
            text: "Hello",
            start_ms: 0,
            end_ms: 100,
            channel: 1,
          },
          {
            id: "word-2",
            text: "world",
            start_ms: 100,
            end_ms: 200,
            channel: 1,
          },
          {
            id: "word-3",
            text: "Again",
            start_ms: 200,
            end_ms: 300,
            channel: 1,
          },
        ]),
        speaker_hints_json: "[]",
      },
    ]);

    await updateTranscriptSegmentText({
      transcriptId: "transcript-1",
      wordIds: ["word-1", "word-2"],
      text: "Hello brave new world",
    });

    const statement = mocks.executeTransaction.mock.calls[0]?.[0]?.[0];
    expect(JSON.parse(String(statement?.params[0]))).toEqual([
      expect.objectContaining({
        id: "word-1",
        text: "Hello",
        start_ms: 0,
        end_ms: 100,
      }),
      expect.objectContaining({
        id: "word-2",
        text: "brave new world",
        start_ms: 100,
        end_ms: 200,
      }),
      expect.objectContaining({ id: "word-3", text: "Again" }),
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
