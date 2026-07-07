import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SegmentHeader } from "./segment-header";

import type { Segment } from "~/stt/live-segment";

const { useStoreMock, useValueMock, useSliceRowIdsMock, useTableMock } =
  vi.hoisted(() => ({
    useStoreMock: vi.fn(),
    useValueMock: vi.fn(),
    useSliceRowIdsMock: vi.fn(),
    useTableMock: vi.fn(),
  }));

vi.mock("./speaker-assign", () => ({
  SpeakerAssignPopover: ({ label }: { label: string }) => (
    <button type="button">{label}</button>
  ),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  INDEXES: {
    sessionParticipantsBySession: "sessionParticipantsBySession",
  },
  STORE_ID: "main",
  UI: {
    useSliceRowIds: useSliceRowIdsMock,
    useStore: useStoreMock,
    useTable: useTableMock,
    useValue: useValueMock,
  },
}));

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
  useStoreMock.mockReturnValue(undefined);
  useValueMock.mockReturnValue(undefined);
  useSliceRowIdsMock.mockReturnValue([]);
  useTableMock.mockReturnValue(undefined);
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

  it("labels remote live segments as the unique other participant", () => {
    useValueMock.mockReturnValue("self");
    useSliceRowIdsMock.mockReturnValue(["mapping-self", "mapping-remote"]);
    useTableMock.mockImplementation((table: string) => {
      if (table === "mapping_session_participant") {
        return {
          "mapping-remote": { human_id: "remote", session_id: "session-1" },
          "mapping-self": { human_id: "self", session_id: "session-1" },
        };
      }

      if (table === "humans") {
        return {
          remote: { name: "Artem" },
          self: { name: "John" },
        };
      }
    });
    useStoreMock.mockReturnValue({
      getCell: (table: string, rowId: string, cell: string) => {
        if (
          table === "transcripts" &&
          rowId === "transcript-1" &&
          cell === "session_id"
        ) {
          return "session-1";
        }

        if (
          table === "mapping_session_participant" &&
          rowId === "mapping-self" &&
          cell === "session_id"
        ) {
          return "session-1";
        }

        if (
          table === "mapping_session_participant" &&
          rowId === "mapping-self" &&
          cell === "human_id"
        ) {
          return "self";
        }

        if (
          table === "mapping_session_participant" &&
          rowId === "mapping-remote" &&
          cell === "session_id"
        ) {
          return "session-1";
        }

        if (
          table === "mapping_session_participant" &&
          rowId === "mapping-remote" &&
          cell === "human_id"
        ) {
          return "remote";
        }

        return undefined;
      },
      getValue: (key: string) => (key === "user_id" ? "self" : undefined),
      getRow: (_table: string, rowId: string) => ({
        name: rowId === "self" ? "John" : rowId === "remote" ? "Artem" : "",
      }),
      forEachRow: (_table: string, callback: (rowId: string) => void) => {
        callback("mapping-self");
        callback("mapping-remote");
      },
    });

    render(
      <SegmentHeader
        transcriptId="transcript-1"
        segment={
          {
            id: "segment-1",
            key: {
              channel: "RemoteParty",
              speaker_index: 0,
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
          } as Segment
        }
      />,
    );

    expect(screen.getByRole("button", { name: "Artem" })).toBeTruthy();
  });

  it("updates cached remote labels when session participants change", () => {
    let participantMappingIds = ["mapping-self", "mapping-remote"];
    let mappingTable: Record<string, { human_id: string; session_id: string }> =
      {
        "mapping-remote": { human_id: "remote", session_id: "session-1" },
        "mapping-self": { human_id: "self", session_id: "session-1" },
      };
    let humansTable: Record<string, { name: string }> = {
      remote: { name: "Artem" },
      self: { name: "John" },
    };
    const store = {
      getCell: (table: string, rowId: string, cell: string) => {
        if (
          table === "transcripts" &&
          rowId === "transcript-1" &&
          cell === "session_id"
        ) {
          return "session-1";
        }

        if (table === "mapping_session_participant") {
          return mappingTable[rowId]?.[
            cell as keyof (typeof mappingTable)[string]
          ];
        }

        return undefined;
      },
      getValue: (key: string) => (key === "user_id" ? "self" : undefined),
      getRow: (_table: string, rowId: string) => {
        return humansTable[rowId] ?? { name: "" };
      },
      forEachRow: (_table: string, callback: (rowId: string) => void) => {
        participantMappingIds.forEach(callback);
      },
    };
    useStoreMock.mockReturnValue(store);
    useValueMock.mockReturnValue("self");
    useSliceRowIdsMock.mockImplementation(() => participantMappingIds);
    useTableMock.mockImplementation((table: string) => {
      if (table === "mapping_session_participant") {
        return mappingTable;
      }

      if (table === "humans") {
        return humansTable;
      }
    });
    const segment = {
      id: "segment-1",
      key: {
        channel: "RemoteParty",
        speaker_index: 0,
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

    const { rerender } = render(
      <SegmentHeader transcriptId="transcript-1" segment={segment} />,
    );

    expect(screen.getByRole("button", { name: "Artem" })).toBeTruthy();

    participantMappingIds = [
      "mapping-self",
      "mapping-remote",
      "mapping-remote-2",
    ];
    mappingTable = {
      ...mappingTable,
      "mapping-remote-2": {
        human_id: "remote-2",
        session_id: "session-1",
      },
    };
    humansTable = {
      ...humansTable,
      "remote-2": { name: "Taylor" },
    };

    rerender(<SegmentHeader transcriptId="transcript-1" segment={segment} />);

    expect(screen.getByRole("button", { name: "Speaker 1" })).toBeTruthy();
  });
});
