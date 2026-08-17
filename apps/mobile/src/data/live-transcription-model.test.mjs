import assert from "node:assert/strict";
import test from "node:test";

import { parseHostedTranscriptionMessage } from "./live-transcription-model.ts";

test("maps partial and finalized hosted transcript responses", () => {
  assert.deepEqual(
    parseHostedTranscriptionMessage(
      JSON.stringify({
        type: "Results",
        is_final: false,
        start: 0,
        duration: 1,
        channel_index: [0, 1],
        channel: { alternatives: [{ transcript: "hello", words: [] }] },
      }),
      "transcript",
    ),
    [{ type: "partial", text: "hello" }],
  );

  const [event] = parseHostedTranscriptionMessage(
    JSON.stringify({
      type: "Results",
      is_final: true,
      start: 1,
      duration: 1,
      channel_index: [0, 1],
      channel: {
        alternatives: [
          {
            transcript: "Hello world.",
            words: [
              {
                word: "hello",
                punctuated_word: "Hello",
                start: 1,
                end: 1.4,
                speaker: 2,
              },
              {
                word: "world",
                punctuated_word: "world.",
                start: 1.4,
                end: 2,
              },
            ],
          },
        ],
      },
    }),
    "transcript",
  );

  assert.equal(event?.type, "final");
  assert.deepEqual(event?.words, [
    {
      id: "transcript:0:1000:0",
      text: "Hello",
      start_ms: 1000,
      end_ms: 1400,
      channel: 0,
      state: "final",
      speaker_index: 2,
    },
    {
      id: "transcript:0:1000:1",
      text: "world.",
      start_ms: 1400,
      end_ms: 2000,
      channel: 0,
      state: "final",
    },
  ]);
  assert.equal(event?.hints.length, 1);
});

test("synthesizes timings when a provider omits word details", () => {
  const [event] = parseHostedTranscriptionMessage(
    JSON.stringify({
      type: "Results",
      is_final: true,
      start: 2,
      duration: 0,
      channel_index: [0, 1],
      channel: {
        alternatives: [{ transcript: "one two", words: [] }],
      },
    }),
    "transcript",
  );

  assert.equal(event?.type, "final");
  assert.deepEqual(
    event?.words.map((word) => [word.text, word.start_ms, word.end_ms]),
    [
      ["one", 2000, 2400],
      ["two", 2400, 2800],
    ],
  );
});

test("handles response batches, terminal messages, and provider errors", () => {
  assert.deepEqual(
    parseHostedTranscriptionMessage(
      JSON.stringify([
        { type: "SpeechStarted" },
        { type: "Metadata" },
        { type: "Error", error_message: "upstream unavailable" },
      ]),
      "transcript",
    ),
    [{ type: "terminal" }, { type: "error", message: "upstream unavailable" }],
  );
});

test("rejects malformed and oversized responses without throwing", () => {
  assert.deepEqual(parseHostedTranscriptionMessage("not-json", "transcript"), [
    { type: "error", message: "Live transcription returned invalid data" },
  ]);
  assert.deepEqual(
    parseHostedTranscriptionMessage("x".repeat(1_000_001), "transcript"),
    [
      {
        type: "error",
        message: "Live transcription response is too large",
      },
    ],
  );
});
