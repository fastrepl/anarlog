import { describe, expect, it } from "vitest";

import { normalizeMentionRow } from "./sqlite-productivity-shadow";

describe("SQLite JSON-domain row normalization", () => {
  it("preserves mention endpoints", () => {
    expect(
      normalizeMentionRow({
        user_id: "user-1",
        source_id: "note-1",
        source_type: "document",
        target_id: "human-1",
        target_type: "human",
      }),
    ).toMatchObject({ source_id: "note-1", target_id: "human-1" });
  });
});
