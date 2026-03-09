import { describe, expect, test } from "vitest";

import { getTranscriptExportData } from "./export-transcript-data";

function createStore(cells: Record<string, unknown>) {
  return {
    getCell: (_tableId: "transcripts", rowId: string, cellId: string) =>
      cells[`${rowId}:${cellId}`],
    getRow: () => ({}),
    getValue: () => undefined,
  };
}

describe("getTranscriptExportData", () => {
  test("reads transcript content in started_at order", () => {
    const store = createStore({
      "late:started_at": 2_000,
      "late:ended_at": 4_000,
      "late:words": JSON.stringify([
        {
          id: "word-2",
          text: "second",
          start_ms: 0,
          end_ms: 500,
          channel: 0,
        },
      ]),
      "late:speaker_hints": "[]",
      "early:started_at": 1_000,
      "early:ended_at": 3_000,
      "early:words": JSON.stringify([
        {
          id: "word-1",
          text: "first",
          start_ms: 0,
          end_ms: 500,
          channel: 0,
        },
      ]),
      "early:speaker_hints": "[]",
    });

    const data = getTranscriptExportData(store as never, ["late", "early"]);

    expect(data.items[0]?.text).toBe("first second");
    expect(data.vttWords[0]?.text).toBe("first second");
  });

  test("keeps words without end_ms in export output", () => {
    const store = createStore({
      "transcript:started_at": 1_000,
      "transcript:ended_at": 61_000,
      "transcript:words": JSON.stringify([
        {
          id: "word-1",
          text: "hello",
          start_ms: 0,
          channel: 0,
        },
      ]),
      "transcript:speaker_hints": "[]",
    });

    const data = getTranscriptExportData(store as never, ["transcript"]);

    expect(data.items).toHaveLength(1);
    expect(data.items[0]?.text).toBe("hello");
    expect(data.vttWords).toHaveLength(1);
    expect(data.vttWords[0]?.text).toBe("hello");
    expect(data.vttWords[0]?.start_ms).toBe(0);
    expect(data.vttWords[0]?.end_ms).toBe(0);
    expect(data.duration).toBe("1m");
  });
});
