import { describe, expect, it } from "vitest";

import { normalizeChatMessageRow } from "./sqlite-chat-shadow";
import {
  normalizeDailyNoteRow,
  normalizeMentionRow,
  normalizeTaskRow,
} from "./sqlite-productivity-shadow";

describe("SQLite JSON-domain row normalization", () => {
  it("preserves structured chat payloads as opaque JSON text", () => {
    expect(
      normalizeChatMessageRow({
        user_id: "user-1",
        created_at: "created",
        chat_group_id: "group-1",
        role: "assistant",
        content: "Done",
        metadata: '{"model":"test"}',
        parts: '[{"type":"text"}]',
        status: "ready",
      }),
    ).toMatchObject({
      metadata: '{"model":"test"}',
      parts: '[{"type":"text"}]',
    });
  });

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

  it("preserves daily note content and mention endpoints", () => {
    expect(
      normalizeDailyNoteRow({
        user_id: "user-1",
        date: "2026-07-11",
        content: '{"type":"doc"}',
      }),
    ).toMatchObject({ date: "2026-07-11", content: '{"type":"doc"}' });
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
