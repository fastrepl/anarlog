import { beforeEach, describe, expect, test, vi } from "vitest";

import { hydrateSessionContent } from "./hydrate";
import { loadSingleSession } from "./load";

import { ok } from "~/store/tinybase/persister/shared";
import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

vi.mock("./load", () => ({
  loadSingleSession: vi.fn(),
}));

vi.mock("~/store/tinybase/persister/shared/paths", () => ({
  getDataDir: vi.fn().mockResolvedValue("/data"),
}));

const loadSingleSessionMock = vi.mocked(loadSingleSession);

describe("hydrateSessionContent", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test("merges loaded session content without removing other sessions", async () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "First",
      raw_md: "",
    });
    store.setRow("sessions", "session-2", {
      user_id: "user-1",
      created_at: "2024-01-02T00:00:00Z",
      title: "Second",
      raw_md: "",
    });

    loadSingleSessionMock.mockResolvedValue(
      ok({
        sessions: {
          "session-1": {
            user_id: "user-1",
            created_at: "2024-01-01T00:00:00Z",
            title: "First",
            raw_md: '{"type":"doc"}',
          },
        },
        mapping_session_participant: {},
        tags: {},
        mapping_tag_session: {},
        transcripts: {},
        enhanced_notes: {},
        session_key_facts: {},
      }),
    );

    await hydrateSessionContent(store, "session-1");

    expect(store.getRowIds("sessions")).toEqual(["session-1", "session-2"]);
    expect(store.getCell("sessions", "session-1", "raw_md")).toBe(
      '{"type":"doc"}',
    );
    expect(store.getCell("sessions", "session-2", "title")).toBe("Second");
  });
});
