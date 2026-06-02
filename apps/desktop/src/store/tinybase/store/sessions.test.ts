import { beforeEach, describe, expect, test, vi } from "vitest";

import { createSession, isSessionEmpty } from "./sessions";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

const analyticsEventMock = vi.hoisted(() => vi.fn());

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: analyticsEventMock,
  },
}));

type Store = Parameters<typeof createSession>[0];

describe("createSession", () => {
  let store: Store;

  beforeEach(() => {
    store = createTestMainStore() as Store;
    store.setValue("user_id", "user-1");
    store.setRow("humans", "user-1", {
      user_id: "user-1",
      name: "John",
      email: "john@example.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    analyticsEventMock.mockClear();
  });

  test("adds the current user as a participant", () => {
    const sessionId = createSession(store);
    const participants = Object.values(
      store.getTable("mapping_session_participant"),
    );

    expect(store.getRow("sessions", sessionId)).toEqual(
      expect.objectContaining({
        user_id: "user-1",
        title: "",
        raw_md: "",
      }),
    );
    expect(participants).toEqual([
      expect.objectContaining({
        user_id: "user-1",
        session_id: sessionId,
        human_id: "user-1",
        source: "manual",
      }),
    ]);
    expect(analyticsEventMock).toHaveBeenCalledWith({
      event: "note_created",
      has_event_id: false,
    });
  });

  test("keeps a note with only the default user participant empty", () => {
    const sessionId = createSession(store);

    expect(isSessionEmpty(store, sessionId)).toBe(true);
  });

  test("treats additional manual participants as note content", () => {
    const sessionId = createSession(store);
    store.setRow("humans", "participant-1", {
      user_id: "user-1",
      name: "Anand",
      email: "anand@example.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: sessionId,
      human_id: "participant-1",
      source: "manual",
    });

    expect(isSessionEmpty(store, sessionId)).toBe(false);
  });
});
