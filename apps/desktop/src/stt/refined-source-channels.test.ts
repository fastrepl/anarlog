import { expect, it } from "vitest";

import {
  restoreRefinedSourceChannels,
  restoreRefinedSourceHints,
} from "./refined-source-channels";

it("recovers original microphone and remote channels after downmixing", () => {
  const previous = [
    { id: "a", text: "Hello", start_ms: 0, end_ms: 500, channel: 0 },
    { id: "b", text: "안녕", start_ms: 500, end_ms: 1000, channel: 1 },
  ];
  const refined = previous.map((word) => ({
    ...word,
    id: `new-${word.id}`,
    channel: 2,
  }));
  expect(
    restoreRefinedSourceChannels(previous, refined).map((word) => word.channel),
  ).toEqual([0, 1]);
});

it("keeps overlapping or rewritten speech mixed", () => {
  const local = {
    id: "a",
    text: "hello",
    start_ms: 0,
    end_ms: 500,
    channel: 0,
  };
  const remote = { ...local, id: "b", channel: 1 };
  const mixed = { ...local, id: "new", channel: 2 };
  expect(restoreRefinedSourceChannels([local, remote], [mixed])).toEqual([
    mixed,
  ]);
  expect(
    restoreRefinedSourceChannels([local], [{ ...mixed, text: "goodbye" }])[0]
      ?.channel,
  ).toBe(2);
  expect(
    restoreRefinedSourceChannels(
      [local],
      [{ ...mixed, start_ms: 490, end_ms: 990 }],
    )[0]?.channel,
  ).toBe(2);
});

it("preserves recovered sources through provider hints in a subsequent render", () => {
  const hints = [
    {
      id: "hint",
      word_id: "a",
      type: "provider_speaker_index",
      value: JSON.stringify({ channel: 2, speaker_index: 7 }),
    },
  ];
  expect(
    JSON.parse(
      restoreRefinedSourceHints(
        [{ id: "a", text: "Hello", start_ms: 0, end_ms: 500, channel: 0 }],
        hints,
      )[0]!.value!,
    ),
  ).toEqual({ channel: 0, speaker_index: 7 });
  expect(
    restoreRefinedSourceHints(
      [{ id: "a", text: "Hello", start_ms: 0, end_ms: 500, channel: 2 }],
      hints,
    ),
  ).toEqual(hints);
});
