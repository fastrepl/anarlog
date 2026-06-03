import { describe, expect, test } from "vitest";

import {
  applyExtractedContactToHuman,
  applyExtractedContacts,
  buildEventContactExtractionContext,
  extractDeterministicContactHints,
} from "./event-contact-extraction";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";
import type { Store } from "~/store/tinybase/store/main";

function createStore(): Store {
  const store = createTestMainStore() as Store;
  store.setValue("user_id", "user-1");
  store.setRow("humans", "user-1", {
    user_id: "user-1",
    created_at: "2026-04-01T00:00:00.000Z",
    name: "John Jeong",
    email: "john@example.com",
    phone: "",
    org_id: "",
    job_title: "",
    linkedin_username: "",
    memo: "",
    pinned: false,
  });
  store.setRow("sessions", "session-1", {
    user_id: "user-1",
    created_at: "2026-04-01T00:00:00.000Z",
    title: "Yongkyun (Daniel) Lee <> john",
    raw_md: "",
    event_json: JSON.stringify({
      tracking_id: "event-tracking-1",
      calendar_id: "calendar-1",
      title: "Yongkyun (Daniel) Lee <> john",
      started_at: "2026-04-21T20:00:00.000Z",
      ended_at: "2026-04-21T20:20:00.000Z",
      is_all_day: false,
      has_recurrence_rules: false,
      description:
        "What:\nYongkyun (Daniel) Lee <> john\n\nWho:\nJohn Jeong - Organizer",
    }),
  });
  store.setRow("mapping_session_participant", "mapping-user", {
    user_id: "user-1",
    session_id: "session-1",
    human_id: "user-1",
    source: "manual",
  });

  return store;
}

describe("event contact extraction", () => {
  test("extracts screenshot-style name hints and matches the participant email", () => {
    const store = createStore();
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "yongkyun.daniel.lee@gmail.com",
      email: "yongkyun.daniel.lee@gmail.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "auto",
    });

    const context = buildEventContactExtractionContext(store, "session-1", {
      tracking_id: "event-tracking-1",
      calendar_id: "calendar-1",
      title: "Yongkyun (Daniel) Lee <> john",
      started_at: "2026-04-21T20:00:00.000Z",
      ended_at: "2026-04-21T20:20:00.000Z",
      is_all_day: false,
      has_recurrence_rules: false,
      description:
        "What:\nYongkyun (Daniel) Lee <> john\n\nWho:\nJohn Jeong - Organizer",
    });

    expect(extractDeterministicContactHints(context)).toEqual([
      {
        name: "Yongkyun (Daniel) Lee",
        email: "yongkyun.daniel.lee@gmail.com",
      },
    ]);
  });

  test("updates an existing email-only contact without creating a duplicate mapping", () => {
    const store = createStore();
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "yongkyun.daniel.lee@gmail.com",
      email: "yongkyun.daniel.lee@gmail.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "auto",
    });

    const result = applyExtractedContacts(
      store,
      "session-1",
      [
        {
          name: "Yongkyun (Daniel) Lee",
          email: "yongkyun.daniel.lee@gmail.com",
        },
      ],
      { userId: "user-1", createdAt: "2026-04-21T20:00:00.000Z" },
    );

    expect(result).toMatchObject({
      created: 0,
      updated: 1,
      linked: 0,
    });
    expect(store.getCell("humans", "human-1", "name")).toBe(
      "Yongkyun (Daniel) Lee",
    );
    expect(
      Object.values(store.getTable("mapping_session_participant")).filter(
        (mapping) => mapping.human_id === "human-1",
      ),
    ).toHaveLength(1);
  });

  test("enriches an existing name-only participant instead of creating a duplicate", () => {
    const store = createStore();
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "Yongkyun (Daniel) Lee",
      email: "",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    });

    const result = applyExtractedContacts(
      store,
      "session-1",
      [
        {
          name: "Yongkyun (Daniel) Lee",
          email: "yongkyun.daniel.lee@gmail.com",
        },
      ],
      { userId: "user-1", createdAt: "2026-04-21T20:00:00.000Z" },
    );

    expect(result).toMatchObject({
      created: 0,
      updated: 1,
      linked: 0,
    });
    expect(store.getCell("humans", "human-1", "email")).toBe(
      "yongkyun.daniel.lee@gmail.com",
    );
    expect(Object.keys(store.getTable("humans"))).toHaveLength(2);
    expect(
      Object.values(store.getTable("mapping_session_participant")).filter(
        (mapping) => mapping.human_id === "human-1",
      ),
    ).toHaveLength(1);
  });

  test("enhances only the selected participant contact", () => {
    const store = createStore();
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "yongkyun.daniel.lee@gmail.com",
      email: "yongkyun.daniel.lee@gmail.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("humans", "human-2", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "other@example.com",
      email: "other@example.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "auto",
    });
    store.setRow("mapping_session_participant", "mapping-2", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-2",
      source: "auto",
    });

    const result = applyExtractedContactToHuman(
      store,
      "session-1",
      "human-1",
      [
        {
          name: "Yongkyun (Daniel) Lee",
          email: "yongkyun.daniel.lee@gmail.com",
        },
        {
          name: "Other Person",
          email: "other@example.com",
        },
      ],
      { userId: "user-1" },
    );

    expect(result).toMatchObject({
      created: 0,
      updated: 1,
      linked: 0,
      matched: true,
    });
    expect(store.getCell("humans", "human-1", "name")).toBe(
      "Yongkyun (Daniel) Lee",
    );
    expect(store.getCell("humans", "human-2", "name")).toBe(
      "other@example.com",
    );
    expect(Object.keys(store.getTable("humans"))).toHaveLength(3);
    expect(
      Object.keys(store.getTable("mapping_session_participant")),
    ).toHaveLength(3);
  });

  test("does not enhance a selected participant from a loose first-name alias", () => {
    const store = createStore();
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "Yongkyun",
      email: "",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "manual",
    });

    const result = applyExtractedContactToHuman(
      store,
      "session-1",
      "human-1",
      [
        {
          name: "Yongkyun Lee",
          email: "yongkyun.lee@example.com",
        },
      ],
      { userId: "user-1" },
    );

    expect(result).toMatchObject({
      updated: 0,
      matched: false,
    });
    expect(store.getCell("humans", "human-1", "email")).toBe("");
  });

  test("treats the current user contact as already up to date", () => {
    const store = createStore();

    const result = applyExtractedContactToHuman(
      store,
      "session-1",
      "user-1",
      [
        {
          name: "John Jeong",
          email: "john@example.com",
        },
      ],
      { userId: "user-1" },
    );

    expect(result).toMatchObject({
      updated: 0,
      skipped: 1,
      matched: true,
    });
    expect(store.getCell("humans", "user-1", "email")).toBe("john@example.com");
  });

  test("creates and links a new contact when no existing contact matches", () => {
    const store = createStore();

    const result = applyExtractedContacts(
      store,
      "session-1",
      [{ name: "Yongkyun (Daniel) Lee" }],
      { userId: "user-1", createdAt: "2026-04-21T20:00:00.000Z" },
    );

    expect(result).toMatchObject({
      created: 1,
      updated: 0,
      linked: 1,
    });

    const createdHuman = Object.values(store.getTable("humans")).find(
      (human) => human.name === "Yongkyun (Daniel) Lee",
    );
    expect(createdHuman).toMatchObject({
      email: "",
      created_at: "2026-04-21T20:00:00.000Z",
    });

    const createdHumanId = Object.entries(store.getTable("humans")).find(
      ([, human]) => human.name === "Yongkyun (Daniel) Lee",
    )?.[0];
    expect(createdHumanId).toBeTruthy();
    expect(
      Object.values(store.getTable("mapping_session_participant")).some(
        (mapping) =>
          mapping.session_id === "session-1" &&
          mapping.human_id === createdHumanId &&
          mapping.source === "manual",
      ),
    ).toBe(true);
  });

  test("does not relink excluded participant mappings", () => {
    const store = createStore();
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "yongkyun.daniel.lee@gmail.com",
      email: "yongkyun.daniel.lee@gmail.com",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    });
    store.setRow("mapping_session_participant", "mapping-1", {
      user_id: "user-1",
      session_id: "session-1",
      human_id: "human-1",
      source: "excluded",
    });

    const result = applyExtractedContacts(
      store,
      "session-1",
      [
        {
          name: "Yongkyun (Daniel) Lee",
          email: "yongkyun.daniel.lee@gmail.com",
        },
      ],
      { userId: "user-1", createdAt: "2026-04-21T20:00:00.000Z" },
    );

    expect(result.linked).toBe(0);
    expect(
      store.getCell("mapping_session_participant", "mapping-1", "source"),
    ).toBe("excluded");
  });
});
