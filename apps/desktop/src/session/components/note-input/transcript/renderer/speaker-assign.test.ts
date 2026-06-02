import { describe, expect, it } from "vitest";

import {
  buildCreateSpeakerParticipantOption,
  buildSpeakerParticipantGroups,
  getAssignmentAnchorWordId,
  type SpeakerParticipantOption,
} from "./speaker-assign";

import type { Segment } from "~/stt/live-segment";

function option(
  id: string,
  name: string,
  overrides: Partial<SpeakerParticipantOption> = {},
): SpeakerParticipantOption {
  return {
    id,
    name,
    isSessionParticipant: false,
    ...overrides,
  };
}

describe("buildSpeakerParticipantGroups", () => {
  it("falls back to contacts when a transcript has no session participants", () => {
    const groups = buildSpeakerParticipantGroups({
      sessionParticipants: [],
      contacts: [option("human-1", "Alice")],
      query: "",
    });

    expect(groups).toEqual([
      {
        title: "Contacts",
        options: [option("human-1", "Alice")],
      },
    ]);
  });

  it("keeps session participants first and excludes duplicate contacts", () => {
    const participant = option("human-1", "Alice", {
      isSessionParticipant: true,
    });
    const groups = buildSpeakerParticipantGroups({
      sessionParticipants: [participant],
      contacts: [option("human-1", "Alice"), option("human-2", "Bob")],
      query: "",
    });

    expect(groups).toEqual([
      {
        title: "Session participants",
        options: [participant],
      },
      {
        title: "Contacts",
        options: [option("human-2", "Bob")],
      },
    ]);
  });
});

describe("buildCreateSpeakerParticipantOption", () => {
  it("creates an add option for a new typed contact name", () => {
    expect(
      buildCreateSpeakerParticipantOption({
        query: "  Charlie  ",
        existingOptions: [option("human-1", "Alice")],
      }),
    ).toEqual({
      id: "new",
      name: "Charlie",
      isSessionParticipant: false,
      isNew: true,
    });
  });

  it("does not create a duplicate add option", () => {
    expect(
      buildCreateSpeakerParticipantOption({
        query: "alice@example.com",
        existingOptions: [
          option("human-1", "Alice", { email: "alice@example.com" }),
        ],
      }),
    ).toBeNull();
  });
});

describe("getAssignmentAnchorWordId", () => {
  it("uses the first available word id in the segment", () => {
    const segment = {
      id: "segment-1",
      key: {
        channel: "RemoteParty",
        speaker_index: 1,
        speaker_human_id: null,
      },
      speaker_label: "Speaker 1",
      start_ms: 0,
      end_ms: 200,
      text: "hello there",
      words: [
        {
          text: "hello",
          start_ms: 0,
          end_ms: 100,
          channel: "RemoteParty",
          is_final: true,
        },
        {
          id: "word-2",
          text: "there",
          start_ms: 100,
          end_ms: 200,
          channel: "RemoteParty",
          is_final: true,
        },
      ],
    } as Segment;

    expect(getAssignmentAnchorWordId(segment)).toBe("word-2");
  });
});
