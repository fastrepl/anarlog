import { describe, expect, test } from "vitest";

import { getChangedSessionIds, parseSessionIdFromPath } from "./changes";

import type {
  ChangedTables,
  TablesContent,
} from "~/store/tinybase/persister/shared";

describe("parseSessionIdFromPath", () => {
  describe("relative paths (from notify events)", () => {
    test("extracts session id from _meta.json", () => {
      expect(parseSessionIdFromPath("sessions/session-123/_meta.json")).toBe(
        "session-123",
      );
    });

    test("extracts session id from transcript.json", () => {
      expect(
        parseSessionIdFromPath("sessions/session-123/transcript.json"),
      ).toBe("session-123");
    });

    test("extracts session id from markdown file", () => {
      expect(parseSessionIdFromPath("sessions/session-123/note.md")).toBe(
        "session-123",
      );
    });

    test("extracts session id from nested folder", () => {
      expect(
        parseSessionIdFromPath("sessions/work/session-123/_meta.json"),
      ).toBe("session-123");
    });

    test("extracts session id from deeply nested folder", () => {
      expect(
        parseSessionIdFromPath(
          "sessions/work/project-a/meetings/session-456/_meta.json",
        ),
      ).toBe("session-456");
    });

    test("handles uuid session ids", () => {
      expect(
        parseSessionIdFromPath(
          "sessions/550e8400-e29b-41d4-a716-446655440000/_meta.json",
        ),
      ).toBe("550e8400-e29b-41d4-a716-446655440000");
    });
  });

  describe("edge cases", () => {
    test("returns null when sessions segment is missing", () => {
      expect(parseSessionIdFromPath("other/session-123/_meta.json")).toBe(null);
    });

    test("returns null for empty path", () => {
      expect(parseSessionIdFromPath("")).toBe(null);
    });

    test("returns null for non-session files", () => {
      expect(parseSessionIdFromPath("sessions/session-123/random.txt")).toBe(
        null,
      );
    });

    test("returns null for directory-only paths", () => {
      expect(parseSessionIdFromPath("sessions/session-123")).toBe(null);
    });
  });
});

describe("getChangedSessionIds", () => {
  describe("direct session changes", () => {
    test("adds changed session ids directly", () => {
      const tables: TablesContent = {};
      const changedTables: ChangedTables = {
        sessions: { "session-1": {}, "session-2": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result).toBeDefined();
      expect(result?.changedSessionIds).toEqual(
        new Set(["session-1", "session-2"]),
      );
      expect(result?.hasUnresolvedDeletions).toBe(false);
    });
  });

  describe("participant changes", () => {
    test("resolves session id from participant", () => {
      const tables: TablesContent = {
        mapping_session_participant: {
          "participant-1": { session_id: "session-1" },
        },
      };
      const changedTables: ChangedTables = {
        mapping_session_participant: { "participant-1": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.changedSessionIds).toEqual(new Set(["session-1"]));
      expect(result?.hasUnresolvedDeletions).toBe(false);
    });

    test("sets hasUnresolvedDeletions when participant not found", () => {
      const tables: TablesContent = {
        mapping_session_participant: {},
      };
      const changedTables: ChangedTables = {
        mapping_session_participant: { "deleted-participant": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.changedSessionIds).toEqual(new Set());
      expect(result?.hasUnresolvedDeletions).toBe(true);
    });
  });

  describe("transcript changes", () => {
    test("resolves session id from transcript", () => {
      const tables: TablesContent = {
        transcripts: { "transcript-1": { session_id: "session-1" } },
      };
      const changedTables: ChangedTables = {
        transcripts: { "transcript-1": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.changedSessionIds).toEqual(new Set(["session-1"]));
      expect(result?.hasUnresolvedDeletions).toBe(false);
    });

    test("sets hasUnresolvedDeletions when transcript not found", () => {
      const tables: TablesContent = { transcripts: {} };
      const changedTables: ChangedTables = {
        transcripts: { "deleted-transcript": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.hasUnresolvedDeletions).toBe(true);
    });
  });

  describe("enhanced note changes", () => {
    test("resolves session id from enhanced note", () => {
      const tables: TablesContent = {
        enhanced_notes: { "note-1": { session_id: "session-1" } },
      };
      const changedTables: ChangedTables = {
        enhanced_notes: { "note-1": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.changedSessionIds).toEqual(new Set(["session-1"]));
      expect(result?.hasUnresolvedDeletions).toBe(false);
    });

    test("sets hasUnresolvedDeletions when note not found", () => {
      const tables: TablesContent = { enhanced_notes: {} };
      const changedTables: ChangedTables = {
        enhanced_notes: { "deleted-note": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.hasUnresolvedDeletions).toBe(true);
    });
  });

  describe("edge cases", () => {
    test("returns undefined when no relevant changes", () => {
      const tables: TablesContent = {};
      const changedTables: ChangedTables = {};

      const result = getChangedSessionIds(tables, changedTables);

      expect(result).toBeUndefined();
    });

    test("returns undefined for unrelated table changes", () => {
      const tables: TablesContent = {};
      const changedTables: ChangedTables = {
        humans: { "human-1": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result).toBeUndefined();
    });

    test("combines changes from multiple sources", () => {
      const tables: TablesContent = {
        mapping_session_participant: {
          "participant-1": { session_id: "session-2" },
        },
        transcripts: { "transcript-1": { session_id: "session-3" } },
      };
      const changedTables: ChangedTables = {
        sessions: { "session-1": {} },
        mapping_session_participant: { "participant-1": {} },
        transcripts: { "transcript-1": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.changedSessionIds).toEqual(
        new Set(["session-1", "session-2", "session-3"]),
      );
    });

    test("deduplicates session ids from multiple changes", () => {
      const tables: TablesContent = {
        mapping_session_participant: {
          "participant-1": { session_id: "session-1" },
        },
        transcripts: { "transcript-1": { session_id: "session-1" } },
      };
      const changedTables: ChangedTables = {
        sessions: { "session-1": {} },
        mapping_session_participant: { "participant-1": {} },
        transcripts: { "transcript-1": {} },
      };

      const result = getChangedSessionIds(tables, changedTables);

      expect(result?.changedSessionIds).toEqual(new Set(["session-1"]));
      expect(result?.changedSessionIds.size).toBe(1);
    });
  });
});
