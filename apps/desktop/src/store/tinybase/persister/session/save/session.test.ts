import { describe, expect, test, vi } from "vitest";

import { buildSessionSaveOps, tablesToSessionMetaMap } from "./session";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));

describe("tablesToSessionMetaMap", () => {
  test("creates meta map with basic session data", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "event-1",
      raw_md: "",
    });

    const result = tablesToSessionMetaMap(store);

    expect(result.get("session-1")).toEqual({
      meta: {
        id: "session-1",
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        title: "Test Session",
        event: undefined,
        participants: [],
        tags: undefined,
      },
      folderPath: "/sessions",
    });
  });

  test("aggregates participants by session", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    });
    store.setRow("mapping_session_participant", "participant-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    });
    store.setRow("mapping_session_participant", "participant-2", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-2",
      source: "auto",
    });

    const result = tablesToSessionMetaMap(store);
    const sessionMeta = result.get("session-1");

    expect(sessionMeta?.meta.participants).toHaveLength(2);
    expect(sessionMeta?.meta.participants).toContainEqual({
      id: "participant-1",
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    });
  });

  test("aggregates tags by session through tag mappings", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    });
    store.setRow("tags", "tag-1", {
      user_id: "user-1",
      name: "work",
    });
    store.setRow("tags", "tag-2", {
      user_id: "user-1",
      name: "important",
    });
    store.setRow("mapping_tag_session", "mapping-1", {
      user_id: "user-1",
      tag_id: "tag-1",
      session_id: "session-1",
    });
    store.setRow("mapping_tag_session", "mapping-2", {
      user_id: "user-1",
      tag_id: "tag-2",
      session_id: "session-1",
    });

    const result = tablesToSessionMetaMap(store);
    const sessionMeta = result.get("session-1");

    expect(sessionMeta?.meta.tags).toContain("work");
    expect(sessionMeta?.meta.tags).toContain("important");
  });

  test("returns undefined tags when session has no tags", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    });

    const result = tablesToSessionMetaMap(store);

    expect(result.get("session-1")?.meta.tags).toBeUndefined();
  });

  test("handles multiple sessions independently", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Session 1",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    });
    store.setRow("sessions", "session-2", {
      user_id: "user-1",
      created_at: "2024-01-02T00:00:00Z",
      title: "Session 2",
      folder_id: "/sessions/work",
      event_json: "",
      raw_md: "",
    });
    store.setRow("mapping_session_participant", "participant-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    });

    const result = tablesToSessionMetaMap(store);

    expect(result.size).toBe(2);
    expect(result.get("session-1")?.meta.participants).toHaveLength(1);
    expect(result.get("session-2")?.meta.participants).toHaveLength(0);
  });

  test("skips participants without session_id", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    });
    store.setRow("mapping_session_participant", "orphan-participant", {
      user_id: "user-1",
      session_id: "",
      human_id: "human-1",
      source: "manual",
    });

    const result = tablesToSessionMetaMap(store);

    expect(result.get("session-1")?.meta.participants).toHaveLength(0);
  });

  test("skips tag mappings without valid tag name", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    });
    store.setRow("tags", "tag-1", {
      user_id: "user-1",
      name: "",
    });
    store.setRow("mapping_tag_session", "mapping-1", {
      user_id: "user-1",
      tag_id: "tag-1",
      session_id: "session-1",
    });

    const result = tablesToSessionMetaMap(store);

    expect(result.get("session-1")?.meta.tags).toBeUndefined();
  });
});

describe("buildSessionSaveOps", () => {
  const dataDir = "/data";

  test("creates json operation for each session", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });

    const ops = buildSessionSaveOps(store, store.getTables(), dataDir);

    expect(ops).toHaveLength(1);
    expect(ops[0].type).toBe("write-json");
    expect((ops[0] as { path: string }).path).toBe(
      "/data/sessions/session-1/_meta.json",
    );
  });

  test("filters by changedSessionIds when provided", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Session 1",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });
    store.setRow("sessions", "session-2", {
      user_id: "user-1",
      created_at: "2024-01-02T00:00:00Z",
      title: "Session 2",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });

    const ops = buildSessionSaveOps(
      store,
      store.getTables(),
      dataDir,
      new Set(["session-1"]),
    );

    expect(ops).toHaveLength(1);
    expect((ops[0] as { path: string }).path).toContain("session-1");
  });

  test("returns empty array when changedSessionIds is empty set", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });

    const ops = buildSessionSaveOps(
      store,
      store.getTables(),
      dataDir,
      new Set(),
    );

    expect(ops).toHaveLength(0);
  });

  test("uses session folder_id for nested folder paths", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "work/meetings",
      event_json: "",
      raw_md: "",
    });

    const ops = buildSessionSaveOps(store, store.getTables(), dataDir);

    expect((ops[0] as { path: string }).path).toBe(
      "/data/sessions/work/meetings/session-1/_meta.json",
    );
  });
});
