import assert from "node:assert/strict";
import { test } from "node:test";

import { transcriptSegments } from "./transcript-model.ts";

const word = (id, text, speaker = 0, start = 0) => ({
  id,
  text,
  start_ms: start,
  end_ms: start + 100,
  channel: 0,
  speaker_index: speaker,
});
const row = (words, deltas = [], hints = []) => ({
  id: "transcript",
  started_at_ms: 1000,
  words_json: JSON.stringify(words),
  pending_deltas_json: JSON.stringify(deltas),
  speaker_hints_json: JSON.stringify(hints),
});

test("live deltas append the full conversation and group consecutive speaker turns", () => {
  const segments = transcriptSegments(
    row(
      [word("a", "Hello")],
      [
        { new_words: [word("b", "there", 0, 100)], replaced_ids: [] },
        { new_words: [word("c", "Hi!", 1, 200)], replaced_ids: [] },
        { new_words: [word("d", "Welcome", 0, 300)], replaced_ids: [] },
      ],
    ),
  );
  assert.deepEqual(
    segments.map(({ text, speaker }) => ({ text, speaker })),
    [
      { text: "Hello there", speaker: "Speaker 1" },
      { text: "Hi!", speaker: "Speaker 2" },
      { text: "Welcome", speaker: "Speaker 1" },
    ],
  );
  assert.equal(segments[0].startMs, 1000);
});

test("revisions replace prior words without duplicating repeated final events", () => {
  const replacement = {
    new_words: [word("b", "Corrected")],
    replaced_ids: ["a"],
  };
  const segments = transcriptSegments(
    row([word("a", "Wrong")], [replacement, replacement]),
  );
  assert.equal(segments.length, 1);
  assert.equal(segments[0].text, "Corrected");
});

test("compacting the saved transcript preserves the exact displayed history", () => {
  const a = word("a", "First");
  const b = word("b", "Second", 1, 100);
  assert.deepEqual(
    transcriptSegments(row([a], [{ new_words: [b], replaced_ids: [] }])),
    transcriptSegments(row([a, b])),
  );
});

test("desktop provider hints and user speaker assignments preserve names and turns", () => {
  const hints = [
    {
      word_id: "a",
      type: "provider_speaker_index",
      value: { speaker_index: 2, channel: 1 },
    },
    {
      word_id: "a",
      type: "automatic_speaker_assignment",
      value: { human_id: "auto" },
    },
    {
      word_id: "a",
      type: "user_speaker_assignment",
      value: JSON.stringify({
        scope: "speaker",
        channel: 1,
        speaker_index: 2,
        human_id: "john",
      }),
    },
    {
      word_id: "b",
      type: "user_speaker_assignment",
      value: { scope: "segment", word_ids: ["b"], human_id: "guest" },
    },
  ];
  const segments = transcriptSegments(
    row([word("a", "One"), word("b", "Two", 0, 100)], [], hints),
    new Map([
      ["auto", "Automatic match"],
      ["john", "John"],
      ["guest", "Guest"],
    ]),
  );
  assert.deepEqual(
    segments.map((s) => s.speaker),
    ["John", "Guest"],
  );
});

test("words without diarization remain readable without inventing an identity", () => {
  assert.equal(
    transcriptSegments(row([{ id: "legacy", text: "Hello" }]))[0].speaker,
    "Speaker",
  );
  assert.deepEqual(transcriptSegments(row([{ id: "empty", text: " " }])), []);
  assert.throws(
    () => transcriptSegments({ ...row([]), words_json: "{}" }),
    /Invalid transcript/,
  );
});

test("a live speaker correction wins over the initial persisted provider hint", () => {
  const initial = word("a", "Hello", 0);
  const revised = word("a", "Hello!", 1);
  const originalHint = {
    word_id: "a",
    type: "provider_speaker_index",
    value: { speaker_index: 0, channel: 0 },
  };
  const live = transcriptSegments(
    row(
      [initial],
      [{ new_words: [revised], replaced_ids: ["a"] }],
      [originalHint],
    ),
  );
  const saved = transcriptSegments(
    row(
      [revised],
      [],
      [{ ...originalHint, value: { speaker_index: 1, channel: 0 } }],
    ),
  );
  assert.deepEqual(live, saved);
  assert.equal(live[0].speaker, "Speaker 2");
});
