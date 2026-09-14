import { describe, expect, it } from "vitest";

import {
  appendSpeakerObservation,
  closeSpeakerContext,
  isSharedMicrophone,
  parseSpeakerContext,
} from "./speaker-context";

const observation = {
  start_ms: 1000,
  end_ms: 11000,
  active_call: true,
  calendar_call: false,
  mic_isolated: true,
  shared_microphone: false,
  title: "John x Alex",
  self_names: ["John"],
  participants: [],
};

describe("speaker capture evidence", () => {
  it("coalesces repeated observations while retaining device boundaries", () => {
    let context = appendSpeakerObservation({ intervals: [] }, observation);
    context = appendSpeakerObservation(context, {
      ...observation,
      start_ms: 6000,
      end_ms: 16000,
    });
    expect(context.intervals).toEqual([{ ...observation, end_ms: 16000 }]);
    context = appendSpeakerObservation(context, {
      ...observation,
      start_ms: 8000,
      end_ms: 18000,
      mic_isolated: false,
    });
    expect(
      context.intervals.map((interval) => [
        interval.start_ms,
        interval.end_ms,
        interval.mic_isolated,
      ]),
    ).toEqual([
      [1000, 8000, true],
      [8000, 18000, false],
    ]);
  });

  it("does not bridge a crash or stopped recording with fresh observations", () => {
    const context = appendSpeakerObservation(
      { intervals: [observation] },
      { ...observation, start_ms: 20000, end_ms: 30000 },
    );
    expect(context.intervals).toHaveLength(2);
    expect(closeSpeakerContext(context, 25000).intervals[1]?.end_ms).toBe(
      25000,
    );
  });

  it("ignores stale async results and preserves a serializable capture history", () => {
    const context = { intervals: [observation] };
    expect(
      appendSpeakerObservation(context, { ...observation, start_ms: 0 }),
    ).toBe(context);
    expect(parseSpeakerContext(JSON.stringify(context))).toEqual(context);
  });

  it("fails closed for missing or malformed persisted evidence", () => {
    for (const json of [null, "{", "{}", '{"intervals":[null,{}]}']) {
      expect(parseSpeakerContext(json)).toEqual({ intervals: [] });
    }
  });

  it("does not mistake a shared or virtual microphone for personal input", () => {
    for (const device of [
      "Jabra Speak 750",
      "Conference Room",
      "BlackHole 2ch",
    ])
      expect(isSharedMicrophone(device)).toBe(true);
    expect(isSharedMicrophone("John’s AirPods Pro")).toBe(false);
    expect(isSharedMicrophone("MacBook Pro Microphone")).toBe(false);
  });
});
