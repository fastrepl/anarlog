import { describe, expect, it } from "vitest";

import {
  normalizeMentionRow,
  normalizeTaskRow,
} from "./sqlite-productivity-shadow";

describe("SQLite JSON-domain row normalization", () => {
  it("preserves task identity, ordering, and body JSON", () => {
    expect(
      normalizeTaskRow({
        user_id: "user-1",
        task_id: "task-1",
        source_id: "session-1",
        source_type: "session",
        source_order: 2,
        status: "todo",
        text_preview: "Follow up",
        body_json: '{"type":"doc"}',
        due_date: "2026-07-12",
      }),
    ).toMatchObject({
      task_id: "task-1",
      source_id: "session-1",
      source_order: 2,
      body_json: '{"type":"doc"}',
    });
  });

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
