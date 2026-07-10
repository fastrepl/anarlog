import { describe, expect, it } from "vitest";

import {
  normalizeEnhancedNoteRow,
  normalizeKeyFactsRow,
} from "./sqlite-document-shadow";
import { normalizeTranscriptRow } from "./sqlite-transcript-shadow";

describe("SQLite session-domain row normalization", () => {
  it("preserves transcript ordering payloads and timing", () => {
    expect(
      normalizeTranscriptRow({
        user_id: "user-1",
        created_at: "2026-07-10T01:00:00Z",
        session_id: "session-1",
        started_at: 100,
        ended_at: 200,
        words: '[{"id":"word-1","text":"hello"}]',
        speaker_hints: '[{"word_id":"word-1","speaker_index":0}]',
        memo_md: "memo",
      }),
    ).toEqual({
      user_id: "user-1",
      created_at: "2026-07-10T01:00:00Z",
      session_id: "session-1",
      started_at: 100,
      ended_at: 200,
      words: '[{"id":"word-1","text":"hello"}]',
      speaker_hints: '[{"word_id":"word-1","speaker_index":0}]',
      memo_md: "memo",
    });
  });

  it("normalizes generated notes without losing template placement", () => {
    expect(
      normalizeEnhancedNoteRow({
        user_id: "user-1",
        session_id: "session-1",
        content: '{"type":"doc"}',
        template_id: "template-1",
        position: 3,
        title: "Summary",
      }),
    ).toMatchObject({
      session_id: "session-1",
      content: '{"type":"doc"}',
      template_id: "template-1",
      position: 3,
      title: "Summary",
    });
  });

  it("preserves key-fact source hashes for regeneration checks", () => {
    expect(
      normalizeKeyFactsRow({
        user_id: "user-1",
        session_id: "session-1",
        created_at: "created",
        updated_at: "updated",
        content: "Fact",
        source_hash: "hash",
      }),
    ).toMatchObject({ content: "Fact", source_hash: "hash" });
  });
});
