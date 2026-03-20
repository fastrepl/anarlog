import { beforeEach, describe, expect, test, vi } from "vitest";

import { extractSessionIdAndFolder, processMetaFile } from "./meta";
import { createEmptyLoadedSessionData, type LoadedSessionData } from "./types";

describe("extractSessionIdAndFolder", () => {
  describe("standard paths", () => {
    test("extracts session id and empty folder from root session path", () => {
      const result = extractSessionIdAndFolder(
        "/data/hyprnote/sessions/session-123/_meta.json",
      );
      expect(result).toEqual({
        sessionId: "session-123",
        folderPath: "/data/hyprnote/sessions",
      });
    });

    test("extracts session id and folder from nested path", () => {
      const result = extractSessionIdAndFolder(
        "/data/hyprnote/sessions/work/session-123/_meta.json",
      );
      expect(result).toEqual({
        sessionId: "session-123",
        folderPath: "/data/hyprnote/sessions/work",
      });
    });

    test("extracts session id and folder from deeply nested path", () => {
      const result = extractSessionIdAndFolder(
        "/data/hyprnote/sessions/work/project-a/meetings/session-123/_meta.json",
      );
      expect(result).toEqual({
        sessionId: "session-123",
        folderPath: "/data/hyprnote/sessions/work/project-a/meetings",
      });
    });
  });

  describe("uuid session ids", () => {
    test("extracts uuid session id", () => {
      const result = extractSessionIdAndFolder(
        "/data/sessions/550e8400-e29b-41d4-a716-446655440000/_meta.json",
      );
      expect(result).toEqual({
        sessionId: "550e8400-e29b-41d4-a716-446655440000",
        folderPath: "/data/sessions",
      });
    });
  });

  describe("different file types", () => {
    test("works with transcript.json files", () => {
      const result = extractSessionIdAndFolder(
        "/data/sessions/session-123/transcript.json",
      );
      expect(result).toEqual({
        sessionId: "session-123",
        folderPath: "/data/sessions",
      });
    });

    test("works with markdown files", () => {
      const result = extractSessionIdAndFolder(
        "/data/sessions/session-123/_summary.md",
      );
      expect(result).toEqual({
        sessionId: "session-123",
        folderPath: "/data/sessions",
      });
    });
  });

  describe("edge cases", () => {
    test("returns empty session id for root path", () => {
      const result = extractSessionIdAndFolder("/_meta.json");
      expect(result.sessionId).toBe("");
    });

    test("handles path with single segment", () => {
      const result = extractSessionIdAndFolder("_meta.json");
      expect(result.sessionId).toBe("");
      expect(result.folderPath).toBe("");
    });

    test("handles empty path", () => {
      const result = extractSessionIdAndFolder("");
      expect(result.sessionId).toBe("");
      expect(result.folderPath).toBe("");
    });

    test("handles path with only directory", () => {
      const result = extractSessionIdAndFolder("/data/sessions/session-123/");
      expect(result.sessionId).toBe("session-123");
      expect(result.folderPath).toBe("/data/sessions");
    });

    test("handles relative path from base (root session)", () => {
      const result = extractSessionIdAndFolder(
        "sessions/session-123/_meta.json",
      );
      expect(result.sessionId).toBe("session-123");
      expect(result.folderPath).toBe("");
    });

    test("handles relative path from base (nested folder)", () => {
      const result = extractSessionIdAndFolder(
        "sessions/work/session-123/_meta.json",
      );
      expect(result.sessionId).toBe("session-123");
      expect(result.folderPath).toBe("work");
    });

    test("handles relative path from base (deeply nested folder)", () => {
      const result = extractSessionIdAndFolder(
        "sessions/work/project-a/session-123/_meta.json",
      );
      expect(result.sessionId).toBe("session-123");
      expect(result.folderPath).toBe("work/project-a");
    });
  });
});

describe("processMetaFile", () => {
  let result: LoadedSessionData;

  beforeEach(() => {
    result = createEmptyLoadedSessionData();
  });

  test("parses meta JSON and creates session entry", () => {
    const eventObj = { tracking_id: "event-1", title: "Test" };
    const content = JSON.stringify({
      id: "session-1",
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      event: eventObj,
      participants: [],
    });

    processMetaFile("/data/sessions/session-1/_meta.json", content, result);

    expect(result.sessions["session-1"]).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/data/sessions",
      event_json: JSON.stringify(eventObj),
      raw_md: "",
    });
  });

  test("creates mapping_session_participant entries", () => {
    const content = JSON.stringify({
      id: "session-1",
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      participants: [
        {
          id: "participant-1",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          human_id: "human-1",
          source: "manual",
        },
      ],
    });

    processMetaFile("/data/sessions/session-1/_meta.json", content, result);

    expect(result.mapping_session_participant["participant-1"]).toEqual({
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    });
  });

  test("creates tags and mapping_tag_session entries", () => {
    const content = JSON.stringify({
      id: "session-1",
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      participants: [],
      tags: ["work", "important"],
    });

    processMetaFile("/data/sessions/session-1/_meta.json", content, result);

    expect(result.tags["work"]).toEqual({
      user_id: "user-1",
      name: "work",
    });
    expect(result.tags["important"]).toEqual({
      user_id: "user-1",
      name: "important",
    });
    expect(result.mapping_tag_session["session-1:work"]).toEqual({
      user_id: "user-1",
      tag_id: "work",
      session_id: "session-1",
    });
  });

  test("handles parse errors gracefully", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    processMetaFile(
      "/data/sessions/session-1/_meta.json",
      "invalid json",
      result,
    );

    expect(Object.keys(result.sessions)).toHaveLength(0);
    expect(consoleSpy).toHaveBeenCalled();
    consoleSpy.mockRestore();
  });
});
