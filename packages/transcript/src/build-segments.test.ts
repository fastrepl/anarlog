import { describe, expect, test } from "vitest";

import { buildSegments, SegmentKey, type WordLike } from ".";

describe("buildSegments", () => {
  const testCases = [
    {
      name: "returns no segments when no words are provided",
      finalWords: [],
      partialWords: [],
      expected: [],
      numSpeakers: undefined,
    },
    {
      name: "simple multi-channel example without merging",
      finalWords: [{ text: "0", start_ms: 0, end_ms: 100, channel: 0 }],
      partialWords: [
        { text: "1", start_ms: 150, end_ms: 200, channel: 0 },
        { text: "2", start_ms: 150, end_ms: 200, channel: 1 },
        { text: "3", start_ms: 210, end_ms: 260, channel: 1 },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: false }),
          ],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [
            expect.objectContaining({ text: "2", isFinal: false }),
            expect.objectContaining({ text: "3", isFinal: false }),
          ],
        }),
      ],
    },
    {
      name: "interleaves same-channel turns across speakers within gap threshold",
      finalWords: [{ text: "0", start_ms: 300, end_ms: 400, channel: 1 }],
      partialWords: [
        { text: "1", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "2", start_ms: 600, end_ms: 700, channel: 0 },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
      ],
    },
    {
      name: "should be always sorted per channel by start_ms",
      finalWords: [{ text: "2", start_ms: 400, end_ms: 450, channel: 0 }],
      partialWords: [
        { text: "0", start_ms: 100, end_ms: 150, channel: 0 },
        { text: "1", start_ms: 250, end_ms: 300, channel: 0 },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [
            expect.objectContaining({ text: "0" }),
            expect.objectContaining({ text: "1" }),
            expect.objectContaining({ text: "2" }),
          ],
        }),
      ],
    },
    {
      name: "does not merge speaker turns once it exceeds the max gap",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "2", start_ms: 2101, end_ms: 2201, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 200, channel: 1 },
      ],
      partialWords: [],
      maxGapMs: 2000,
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
      ],
    },
    {
      name: "merges when gap is exactly at maxGapMs threshold (2000ms)",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 2100, end_ms: 2200, channel: 0 },
      ],
      partialWords: [],
      maxGapMs: 2000,
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [
            expect.objectContaining({ text: "0" }),
            expect.objectContaining({ text: "1" }),
          ],
        }),
      ],
    },
    {
      name: "handles three or more distinct channels",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 250, channel: 1 },
        { text: "2", start_ms: 300, end_ms: 400, channel: 2 },
      ],
      partialWords: [],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 2 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
      ],
    },
    {
      name: "handles single word input",
      finalWords: [{ text: "0", start_ms: 0, end_ms: 100, channel: 0 }],
      partialWords: [],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "0", isFinal: true })],
        }),
      ],
    },
    {
      name: "splits segments by speaker within same channel",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 250, channel: 0 },
        { text: "2", start_ms: 300, end_ms: 400, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 1,
          },
        },
        {
          wordIndex: 2,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0, speaker_index: 0 }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0, speaker_index: 1 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0, speaker_index: 0 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
      ],
    },
    {
      name: "interleaves multiple short turns across channels",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 200, channel: 1 },
        { text: "2", start_ms: 250, end_ms: 300, channel: 0 },
        { text: "3", start_ms: 350, end_ms: 400, channel: 1 },
        { text: "4", start_ms: 450, end_ms: 500, channel: 0 },
      ],
      partialWords: [],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "3" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "4" })],
        }),
      ],
    },
    {
      name: "propagates human id across shared speaker index",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 200, end_ms: 300, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 1,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 1,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 1,
            speaker_human_id: "alice",
          }),
          words: [
            expect.objectContaining({ text: "0" }),
            expect.objectContaining({ text: "1" }),
          ],
        }),
      ],
    },
    {
      name: "infers human id for partial words via last known speaker",
      finalWords: [{ text: "0", start_ms: 0, end_ms: 100, channel: 0 }],
      partialWords: [{ text: "1", start_ms: 150, end_ms: 200, channel: 0 }],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 2,
          },
        },
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "bob",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 2,
            speaker_human_id: "bob",
          }),
          words: [
            expect.objectContaining({ text: "0" }),
            expect.objectContaining({ text: "1" }),
          ],
        }),
      ],
    },
    {
      name: "splits segments when human id changes for same speaker index",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 250, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "bob",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 0,
            speaker_human_id: "alice",
          }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 0,
            speaker_human_id: "bob",
          }),
          words: [expect.objectContaining({ text: "1" })],
        }),
      ],
    },
    {
      name: "auto-assign based on provider speaker index",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 100, end_ms: 200, channel: 1 },
        { text: "2", start_ms: 200, end_ms: 300, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 1,
          },
        },
        {
          wordIndex: 2,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "bob",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 0,
            speaker_human_id: "bob",
          }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1, speaker_index: 1 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 0,
            speaker_human_id: "bob",
          }),
          words: [expect.objectContaining({ text: "2" })],
        }),
      ],
    },
    {
      name: "handles partial-only stream with speaker hints",
      finalWords: [],
      partialWords: [
        { text: "0", start_ms: 0, end_ms: 80, channel: 0 },
        { text: "1", start_ms: 120, end_ms: 200, channel: 0 },
      ],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 3,
          },
        },
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 3,
            speaker_human_id: "alice",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: false }),
            expect.objectContaining({ text: "1", isFinal: false }),
          ],
        }),
      ],
    },
    {
      name: "partial word inherits previous segment key ignoring its own speaker hints",
      finalWords: [{ text: "0", start_ms: 0, end_ms: 90, channel: 0 }],
      partialWords: [{ text: "1", start_ms: 140, end_ms: 220, channel: 0 }],
      speakerHints: [
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 4,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: false }),
          ],
        }),
      ],
    },
    {
      name: "merges using human assignment without provider index",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 140, end_ms: 240, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "alice",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: true }),
          ],
        }),
      ],
    },
    {
      name: "propagates human assignment to partial words without speaker index",
      finalWords: [{ text: "0", start_ms: 0, end_ms: 50, channel: 0 }],
      partialWords: [{ text: "1", start_ms: 100, end_ms: 150, channel: 0 }],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "alice",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: false }),
          ],
        }),
      ],
    },
    {
      name: "splits segments when channel-only human assignment changes",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 50, channel: 0 },
        { text: "1", start_ms: 120, end_ms: 170, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "bob",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "alice",
          }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "bob",
          }),
          words: [expect.objectContaining({ text: "1" })],
        }),
      ],
    },
    {
      name: "retains human assignment across partial-only stream without speaker index",
      finalWords: [],
      partialWords: [
        { text: "0", start_ms: 0, end_ms: 80, channel: 1 },
        { text: "1", start_ms: 120, end_ms: 200, channel: 1 },
      ],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "carol",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 1,
            speaker_human_id: "carol",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: false }),
            expect.objectContaining({ text: "1", isFinal: false }),
          ],
        }),
      ],
    },
    {
      name: "propagates DirectMic channel identity forward",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 400, channel: 0 },
        { text: "1", start_ms: 400, end_ms: 600, channel: 0 },
        { text: "2", start_ms: 600, end_ms: 800, channel: 0 },
        { text: "3", start_ms: 800, end_ms: 1400, channel: 0 },
        { text: "4", start_ms: 1400, end_ms: 2000, channel: 0 },
        { text: "5", start_ms: 4100, end_ms: 4500, channel: 1 },
        { text: "6", start_ms: 4500, end_ms: 4900, channel: 1 },
        { text: "7", start_ms: 4900, end_ms: 5300, channel: 1 },
        { text: "8", start_ms: 5300, end_ms: 5700, channel: 1 },
        { text: "9", start_ms: 5700, end_ms: 6100, channel: 1 },
        { text: "10", start_ms: 8200, end_ms: 8600, channel: 0 },
        { text: "11", start_ms: 8600, end_ms: 9000, channel: 0 },
        { text: "12", start_ms: 9000, end_ms: 9200, channel: 0 },
        { text: "13", start_ms: 9200, end_ms: 9800, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "carol",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "carol",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: true }),
            expect.objectContaining({ text: "2", isFinal: true }),
            expect.objectContaining({ text: "3", isFinal: true }),
            expect.objectContaining({ text: "4", isFinal: true }),
          ],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [
            expect.objectContaining({ text: "5", isFinal: true }),
            expect.objectContaining({ text: "6", isFinal: true }),
            expect.objectContaining({ text: "7", isFinal: true }),
            expect.objectContaining({ text: "8", isFinal: true }),
            expect.objectContaining({ text: "9", isFinal: true }),
          ],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "carol",
          }),
          words: [
            expect.objectContaining({ text: "10", isFinal: true }),
            expect.objectContaining({ text: "11", isFinal: true }),
            expect.objectContaining({ text: "12", isFinal: true }),
            expect.objectContaining({ text: "13", isFinal: true }),
          ],
        }),
      ],
    },
    {
      name: "propagates DirectMic channel identity backward",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 400, channel: 0 },
        { text: "1", start_ms: 400, end_ms: 600, channel: 0 },
        { text: "2", start_ms: 600, end_ms: 800, channel: 0 },
        { text: "3", start_ms: 800, end_ms: 1400, channel: 0 },
        { text: "4", start_ms: 1400, end_ms: 2000, channel: 0 },
        { text: "5", start_ms: 4100, end_ms: 4500, channel: 1 },
        { text: "6", start_ms: 4500, end_ms: 4900, channel: 1 },
        { text: "7", start_ms: 4900, end_ms: 5300, channel: 1 },
        { text: "8", start_ms: 5300, end_ms: 5700, channel: 1 },
        { text: "9", start_ms: 5700, end_ms: 6100, channel: 1 },
        { text: "10", start_ms: 8200, end_ms: 8600, channel: 0 },
        { text: "11", start_ms: 8600, end_ms: 9000, channel: 0 },
        { text: "12", start_ms: 9000, end_ms: 9200, channel: 0 },
        { text: "13", start_ms: 9200, end_ms: 9800, channel: 0 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 11,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "carol",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "carol",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: true }),
            expect.objectContaining({ text: "2", isFinal: true }),
            expect.objectContaining({ text: "3", isFinal: true }),
            expect.objectContaining({ text: "4", isFinal: true }),
          ],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [
            expect.objectContaining({ text: "5", isFinal: true }),
            expect.objectContaining({ text: "6", isFinal: true }),
            expect.objectContaining({ text: "7", isFinal: true }),
            expect.objectContaining({ text: "8", isFinal: true }),
            expect.objectContaining({ text: "9", isFinal: true }),
          ],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_human_id: "carol",
          }),
          words: [
            expect.objectContaining({ text: "10", isFinal: true }),
            expect.objectContaining({ text: "11", isFinal: true }),
            expect.objectContaining({ text: "12", isFinal: true }),
            expect.objectContaining({ text: "13", isFinal: true }),
          ],
        }),
      ],
    },
    {
      name: "propagates RemoteParty identity when numSpeakers is 2",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 1 },
        { text: "1", start_ms: 200, end_ms: 300, channel: 1 },
      ],
      partialWords: [],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "remote",
          },
        },
      ],
      numSpeakers: 2,
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 1,
            speaker_human_id: "remote",
          }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: true }),
          ],
        }),
      ],
    },
    {
      name: "places partial words after interleaving speaker turns",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 220, channel: 1 },
      ],
      partialWords: [{ text: "2", start_ms: 230, end_ms: 300, channel: 0 }],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "0", isFinal: true })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "1", isFinal: true })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "2", isFinal: false })],
        }),
      ],
    },
    {
      name: "respects custom maxGapMs value",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 500, end_ms: 600, channel: 0 },
        { text: "2", start_ms: 1700, end_ms: 1800, channel: 0 },
      ],
      partialWords: [],
      maxGapMs: 1000,
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [
            expect.objectContaining({ text: "0" }),
            expect.objectContaining({ text: "1" }),
          ],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
      ],
    },
    {
      name: "partial words inherit speaker_index and interleave across channels",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 250, channel: 1 },
      ],
      partialWords: [
        { text: "2", start_ms: 300, end_ms: 400, channel: 0 },
        { text: "3", start_ms: 450, end_ms: 550, channel: 1 },
      ],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 1,
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0, speaker_index: 0 }),
          words: [expect.objectContaining({ text: "0", isFinal: true })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1, speaker_index: 1 }),
          words: [expect.objectContaining({ text: "1", isFinal: true })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0, speaker_index: 0 }),
          words: [expect.objectContaining({ text: "2", isFinal: false })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1, speaker_index: 1 }),
          words: [expect.objectContaining({ text: "3", isFinal: false })],
        }),
      ],
    },
    {
      name: "partial words inherit full identity and interleave across channels",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 150, end_ms: 250, channel: 1 },
      ],
      partialWords: [
        { text: "2", start_ms: 300, end_ms: 400, channel: 0 },
        { text: "3", start_ms: 450, end_ms: 550, channel: 1 },
        { text: "4", start_ms: 600, end_ms: 700, channel: 0 },
      ],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 5,
          },
        },
        {
          wordIndex: 0,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "alice",
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 7,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "user_speaker_assignment" as const,
            human_id: "bob",
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 5,
            speaker_human_id: "alice",
          }),
          words: [expect.objectContaining({ text: "0", isFinal: true })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 1,
            speaker_index: 7,
            speaker_human_id: "bob",
          }),
          words: [expect.objectContaining({ text: "1", isFinal: true })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 5,
            speaker_human_id: "alice",
          }),
          words: [expect.objectContaining({ text: "2", isFinal: false })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 1,
            speaker_index: 7,
            speaker_human_id: "bob",
          }),
          words: [expect.objectContaining({ text: "3", isFinal: false })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({
            channel: 0,
            speaker_index: 5,
            speaker_human_id: "alice",
          }),
          words: [expect.objectContaining({ text: "4", isFinal: false })],
        }),
      ],
    },
    {
      name: "partial word with intermittent speaker hint stays in previous segment",
      finalWords: [{ text: "0", start_ms: 0, end_ms: 100, channel: 0 }],
      partialWords: [{ text: "1", start_ms: 150, end_ms: 250, channel: 0 }],
      speakerHints: [
        {
          wordIndex: 0,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 0,
          },
        },
        {
          wordIndex: 1,
          data: {
            type: "provider_speaker_index" as const,
            speaker_index: 1,
          },
        },
      ],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0, speaker_index: 0 }),
          words: [
            expect.objectContaining({ text: "0", isFinal: true }),
            expect.objectContaining({ text: "1", isFinal: false }),
          ],
        }),
      ],
    },
    {
      name: "overlapping channels produce interleaved segments",
      finalWords: [
        { text: "0", start_ms: 0, end_ms: 100, channel: 0 },
        { text: "1", start_ms: 50, end_ms: 150, channel: 1 },
        { text: "2", start_ms: 200, end_ms: 300, channel: 0 },
        { text: "3", start_ms: 250, end_ms: 350, channel: 1 },
      ],
      partialWords: [],
      expected: [
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "0" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "1" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 0 }),
          words: [expect.objectContaining({ text: "2" })],
        }),
        expect.objectContaining({
          key: SegmentKey.make({ channel: 1 }),
          words: [expect.objectContaining({ text: "3" })],
        }),
      ],
    },
  ];

  test.each(testCases)(
    "$name",
    ({
      finalWords,
      partialWords,
      speakerHints,
      expected,
      maxGapMs,
      numSpeakers,
    }) => {
      finalWords.forEach((word) => expect(word.channel).toBeLessThanOrEqual(2));
      partialWords.forEach((word) =>
        expect(word.channel).toBeLessThanOrEqual(2),
      );

      const options =
        maxGapMs !== undefined || numSpeakers !== undefined
          ? {
              ...(maxGapMs !== undefined && { maxGapMs }),
              ...(numSpeakers !== undefined && { numSpeakers }),
            }
          : undefined;

      const segments = buildSegments(
        finalWords,
        partialWords,
        speakerHints,
        options,
      );
      expect(segments).toEqual(expected);
    },
  );
});
