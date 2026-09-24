import { describe, expect, it } from "vitest";

import {
  applyZoomClientEvent,
  createZoomSpeakerTimeline,
  wordIdsByParticipant,
} from "./speaker-timeline";

import type { WordWithId } from "~/stt/types";

const word = (id: string, start_ms: number, end_ms: number): WordWithId => ({
  id,
  text: id,
  start_ms,
  end_ms,
  channel: 1,
});

describe("zoom speaker timeline", () => {
  it("attributes words to the dominant active speaker", () => {
    const timeline = createZoomSpeakerTimeline();
    applyZoomClientEvent(
      timeline,
      {
        type: "state_changed",
        data: { session_id: "z", state: "capturing", reason: null },
      },
      10_000,
    );
    applyZoomClientEvent(timeline, {
      type: "participant_upserted",
      data: {
        session_id: "z",
        participant: { id: "a", display_name: "Ada", email: null },
      },
    });
    for (const [at_ms, participant_ids] of [
      [0, ["a"]],
      [1000, ["b"]],
      [2000, []],
    ] as const) {
      applyZoomClientEvent(timeline, {
        type: "active_speakers",
        data: {
          session_id: "z",
          speakers: { at_ms, participant_ids: [...participant_ids] },
        },
      });
    }

    // Transcript started 500ms after capture; word times are transcript-relative.
    const grouped = wordIdsByParticipant(
      [word("w1", 0, 400), word("w2", 600, 1300), word("w3", 1700, 2000)],
      10_500,
      timeline,
    );

    expect([...grouped]).toEqual([
      ["a", ["w1"]],
      ["b", ["w2"]],
    ]);
  });

  it("assigns nothing before capture started", () => {
    const timeline = createZoomSpeakerTimeline();
    applyZoomClientEvent(timeline, {
      type: "active_speakers",
      data: { session_id: "z", speakers: { at_ms: 0, participant_ids: ["a"] } },
    });
    expect(wordIdsByParticipant([word("w", 0, 100)], 0, timeline).size).toBe(0);
  });
});
