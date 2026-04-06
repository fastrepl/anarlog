import { describe, expect, it } from "vitest";

import { upsertSpeakerAssignment } from "./utils";

import type { SegmentKey } from "~/stt/live-segment";

type TranscriptRow = {
  words?: string;
  speaker_hints?: string;
};

function createStore(row: TranscriptRow) {
  const transcript = {
    words: row.words ?? JSON.stringify([]),
    speaker_hints: row.speaker_hints ?? JSON.stringify([]),
  };

  return {
    getCell: (
      tableId: "transcripts",
      rowId: string,
      cellId: "words" | "speaker_hints",
    ) => {
      if (tableId !== "transcripts" || rowId !== "transcript-1") {
        return undefined;
      }

      return transcript[cellId];
    },
    setCell: (
      tableId: "transcripts",
      rowId: string,
      cellId: "words" | "speaker_hints",
      value: string,
    ) => {
      if (tableId !== "transcripts" || rowId !== "transcript-1") {
        return;
      }

      transcript[cellId] = value;
    },
  };
}

function remoteSpeakerKey(speakerIndex: number | null): SegmentKey {
  return {
    channel: "RemoteParty",
    speaker_index: speakerIndex,
    speaker_human_id: null,
  } as SegmentKey;
}

describe("upsertSpeakerAssignment", () => {
  it("removes a stale channel-wide assignment when reassigning a speaker", () => {
    const store = createStore({
      words: JSON.stringify([
        {
          id: "old-word",
          text: " hello",
          start_ms: 0,
          end_ms: 100,
          channel: 1,
        },
        {
          id: "new-word",
          text: " there",
          start_ms: 100,
          end_ms: 200,
          channel: 1,
        },
      ]),
      speaker_hints: JSON.stringify([
        {
          id: "old-word:user_speaker_assignment",
          word_id: "old-word",
          type: "user_speaker_assignment",
          value: JSON.stringify({ human_id: "alice" }),
        },
        {
          id: "new-word:provider_speaker_index",
          word_id: "new-word",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 2 }),
        },
      ]),
    });

    upsertSpeakerAssignment(
      store,
      "transcript-1",
      remoteSpeakerKey(2),
      "bob",
      "new-word",
    );

    expect(
      JSON.parse(
        store.getCell("transcripts", "transcript-1", "speaker_hints") as string,
      ),
    ).toEqual([
      {
        id: "new-word:provider_speaker_index",
        word_id: "new-word",
        type: "provider_speaker_index",
        value: JSON.stringify({ channel: 1, speaker_index: 2 }),
      },
      {
        id: "new-word:user_speaker_assignment",
        word_id: "new-word",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "bob" }),
      },
    ]);
  });

  it("keeps other speaker assignments on the same channel", () => {
    const store = createStore({
      words: JSON.stringify([
        {
          id: "speaker-1-word",
          text: " first",
          start_ms: 0,
          end_ms: 100,
          channel: 1,
        },
        {
          id: "speaker-2-word-old",
          text: " second",
          start_ms: 100,
          end_ms: 200,
          channel: 1,
        },
        {
          id: "speaker-2-word-new",
          text: " later",
          start_ms: 200,
          end_ms: 300,
          channel: 1,
        },
      ]),
      speaker_hints: JSON.stringify([
        {
          id: "speaker-1-word:provider_speaker_index",
          word_id: "speaker-1-word",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 1 }),
        },
        {
          id: "speaker-1-word:user_speaker_assignment",
          word_id: "speaker-1-word",
          type: "user_speaker_assignment",
          value: JSON.stringify({ human_id: "alice" }),
        },
        {
          id: "speaker-2-word-old:provider_speaker_index",
          word_id: "speaker-2-word-old",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 2 }),
        },
        {
          id: "speaker-2-word-old:user_speaker_assignment",
          word_id: "speaker-2-word-old",
          type: "user_speaker_assignment",
          value: JSON.stringify({ human_id: "bob" }),
        },
        {
          id: "speaker-2-word-new:provider_speaker_index",
          word_id: "speaker-2-word-new",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 2 }),
        },
      ]),
    });

    upsertSpeakerAssignment(
      store,
      "transcript-1",
      remoteSpeakerKey(2),
      "carol",
      "speaker-2-word-new",
    );

    expect(
      JSON.parse(
        store.getCell("transcripts", "transcript-1", "speaker_hints") as string,
      ),
    ).toEqual([
      {
        id: "speaker-1-word:provider_speaker_index",
        word_id: "speaker-1-word",
        type: "provider_speaker_index",
        value: JSON.stringify({ channel: 1, speaker_index: 1 }),
      },
      {
        id: "speaker-1-word:user_speaker_assignment",
        word_id: "speaker-1-word",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "alice" }),
      },
      {
        id: "speaker-2-word-old:provider_speaker_index",
        word_id: "speaker-2-word-old",
        type: "provider_speaker_index",
        value: JSON.stringify({ channel: 1, speaker_index: 2 }),
      },
      {
        id: "speaker-2-word-new:provider_speaker_index",
        word_id: "speaker-2-word-new",
        type: "provider_speaker_index",
        value: JSON.stringify({ channel: 1, speaker_index: 2 }),
      },
      {
        id: "speaker-2-word-new:user_speaker_assignment",
        word_id: "speaker-2-word-new",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "carol" }),
      },
    ]);
  });
});
