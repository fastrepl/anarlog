import { describe, expect, it } from "vitest";

import { applyStoredTranscriptEdits } from "./transcript-edits";

describe("applyStoredTranscriptEdits", () => {
  it("applies mobile edits by transcript and stable segment id", () => {
    const segments = [
      { id: "stable-id", text: "deploy to stagin", start_ms: 0 },
      { id: "untouched", text: "keep this", start_ms: 100 },
    ];

    expect(
      applyStoredTranscriptEdits(segments, [
        {
          transcriptId: "transcript-1",
          metadataJson: JSON.stringify({
            transcript_edits: {
              "transcript-1:stable-id": { text: "deploy to staging" },
              "transcript-2:stable-id": { text: "wrong transcript" },
            },
          }),
        },
      ]),
    ).toEqual([
      { id: "stable-id", text: "deploy to staging", start_ms: 0 },
      { id: "untouched", text: "keep this", start_ms: 100 },
    ]);
    expect(segments[0].text).toBe("deploy to stagin");
  });

  it("ignores malformed edit metadata", () => {
    const segments = [{ id: "stable-id", text: "recognized" }];
    expect(
      applyStoredTranscriptEdits(segments, [
        { transcriptId: "transcript-1", metadataJson: "not-json" },
      ]),
    ).toBe(segments);
  });
});
