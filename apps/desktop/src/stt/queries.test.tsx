import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
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
  useSessionParticipantHumanIds,
  useSessionTranscripts,
  useTranscript,
  useTranscriptHumans,
} from "./queries";

describe("transcript SQLite queries", () => {
  beforeEach(() => {
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
});
