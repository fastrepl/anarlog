import { createMergeableStore } from "tinybase/with-schemas";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA } from "@hypr/store";

import type { Store } from "./main";
import { createAdHocSession } from "./sessions";

const { analyticsEventMock } = vi.hoisted(() => ({
  analyticsEventMock: vi.fn(),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: analyticsEventMock,
  },
}));

function createTestStore(): Store {
  return createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value) as Store;
}

describe("createAdHocSession", () => {
  let store: Store;

  beforeEach(() => {
    vi.clearAllMocks();
    store = createTestStore();
  });

  test("adds the current user as a participant", () => {
    store.setValue("user_id", "user-1");
    store.setRow("humans", "user-1", {
      user_id: "user-1",
      name: "John",
      email: "john@example.com",
      org_id: "",
      pinned: false,
    });

    const sessionId = createAdHocSession(store);
    const mappingIds = store.getRowIds("mapping_session_participant");

    expect(mappingIds).toHaveLength(1);
    expect(
      store.getRow("mapping_session_participant", mappingIds[0]),
    ).toMatchObject({
      user_id: "user-1",
      session_id: sessionId,
      human_id: "user-1",
      source: "manual",
    });
  });

  test("creates the current user's human row when missing", () => {
    store.setValue("user_id", "user-1");

    createAdHocSession(store);

    expect(store.getRow("humans", "user-1")).toMatchObject({
      user_id: "user-1",
      name: "",
      email: "",
      org_id: "",
      pinned: false,
    });
  });
});
