import { generateText } from "ai";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  applyExtractedContactToHuman,
  applyExtractedContacts,
  buildEventContactExtractionContext,
  extractEventContacts,
} from "./event-contact-extraction";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";
import type { Store } from "~/store/tinybase/store/main";

const mocks = vi.hoisted(() => ({
  renderTemplate: vi.fn(),
}));

vi.mock("ai", () => ({
  generateText: vi.fn(),
}));

vi.mock("@hypr/plugin-template", () => ({
  commands: {
    render: mocks.renderTemplate,
  },
}));

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
  beforeEach(() => {
    vi.mocked(generateText).mockReset();
    mocks.renderTemplate.mockReset();
    mocks.renderTemplate.mockImplementation(async (template: unknown) => {
      if (
        template &&
        typeof template === "object" &&
        "eventContactSystem" in template
      ) {
        return { status: "success", data: "# System prompt" };
      }

      if (
        template &&
        typeof template === "object" &&
        "eventContactUser" in template
      ) {
        return { status: "success", data: "# User prompt" };
      }

      return { status: "error", error: "Unexpected template" };
    });
  });

  test("extracts contacts from model JSON and matches the participant email", async () => {
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

    vi.mocked(generateText).mockResolvedValue({
      text: JSON.stringify({
        contacts: [{ name: "Yongkyun (Daniel) Lee", email: null }],
      }),
    } as any);

    await expect(
      extractEventContacts({ model: {} as any, context }),
    ).resolves.toEqual({
      source: "model",
      contacts: [
        {
          name: "Yongkyun (Daniel) Lee",
          email: "yongkyun.daniel.lee@gmail.com",
        },
      ],
    });
    expect(generateText).toHaveBeenCalledWith(
      expect.objectContaining({
        model: {},
        system: "# System prompt",
        prompt: "# User prompt",
      }),
    );
    expect(vi.mocked(generateText).mock.calls[0]?.[0]).not.toHaveProperty(
      "output",
    );
  });

  test("enhances an invite title name from model JSON when it matches one participant email", async () => {
    const store = createStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2026-06-16T05:00:00.000Z",
      title: "Tom Yang <> john",
      raw_md: "",
      event_json: JSON.stringify({
        tracking_id: "event-tracking-1",
        calendar_id: "calendar-1",
        title: "Tom Yang <> john",
        started_at: "2026-06-16T05:00:00.000Z",
        ended_at: "2026-06-16T05:20:00.000Z",
        is_all_day: false,
        has_recurrence_rules: false,
        description: "What:\nTom Yang <> john\n\nWho:\nJohn Jeong - Organizer",
      }),
    });
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-06-16T05:00:00.000Z",
      name: "tom@kestroll.com",
      email: "tom@kestroll.com",
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
      title: "Tom Yang <> john",
      started_at: "2026-06-16T05:00:00.000Z",
      ended_at: "2026-06-16T05:20:00.000Z",
      is_all_day: false,
      has_recurrence_rules: false,
      description: "What:\nTom Yang <> john\n\nWho:\nJohn Jeong - Organizer",
    });

    vi.mocked(generateText).mockResolvedValue({
      text: JSON.stringify({
        contacts: [{ name: "Tom Yang", email: null, companyName: "Kestroll" }],
      }),
    } as any);

    const extraction = await extractEventContacts({
      model: {} as any,
      context,
    });
    const result = applyExtractedContactToHuman(
      store,
      "session-1",
      "human-1",
      extraction.contacts,
      { userId: "user-1" },
    );

    expect(extraction.contacts).toEqual([
      {
        name: "Tom Yang",
        email: "tom@kestroll.com",
        companyName: "Kestroll",
      },
    ]);
    expect(result).toMatchObject({
      updated: 1,
      matched: true,
    });
    expect(store.getCell("humans", "human-1", "name")).toBe("Tom Yang");
    const organizations = store.getTable("organizations");
    const organizationEntry = Object.entries(organizations).find(
      ([, organization]) => organization.name === "Kestroll",
    );
    expect(organizationEntry).toBeTruthy();
    expect(store.getCell("humans", "human-1", "org_id")).toBe(
      organizationEntry?.[0],
    );
  });

  test("reuses an existing organization when applying extracted company", () => {
    const store = createStore();
    store.setRow("organizations", "org-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "Kestroll",
      pinned: false,
    });
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "tom@kestroll.com",
      email: "tom@kestroll.com",
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

    const result = applyExtractedContactToHuman(
      store,
      "session-1",
      "human-1",
      [
        {
          name: "Tom Yang",
          email: "tom@kestroll.com",
          companyName: "Kestroll",
        },
      ],
      { userId: "user-1" },
    );

    expect(result).toMatchObject({
      updated: 1,
      matched: true,
    });
    expect(store.getCell("humans", "human-1", "org_id")).toBe("org-1");
    expect(Object.keys(store.getTable("organizations"))).toHaveLength(1);
  });

  test("does not create an unused organization when bulk updating a contact that already has one", () => {
    const store = createStore();
    store.setRow("organizations", "org-existing", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "Existing Co",
      pinned: false,
    });
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "tom@kestroll.com",
      email: "tom@kestroll.com",
      phone: "",
      org_id: "org-existing",
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
          name: "Tom Yang",
          email: "tom@kestroll.com",
          companyName: "Kestroll",
        },
      ],
      { userId: "user-1", createdAt: "2026-04-21T20:00:00.000Z" },
    );

    expect(result).toMatchObject({
      created: 0,
      updated: 1,
      linked: 0,
    });
    expect(store.getCell("humans", "human-1", "name")).toBe("Tom Yang");
    expect(store.getCell("humans", "human-1", "org_id")).toBe("org-existing");
    expect(Object.keys(store.getTable("organizations"))).toHaveLength(1);
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

  test("does not create an unused organization when enhancing a contact that already has one", () => {
    const store = createStore();
    store.setRow("organizations", "org-existing", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "Existing Co",
      pinned: false,
    });
    store.setRow("humans", "human-1", {
      user_id: "user-1",
      created_at: "2026-04-01T00:00:00.000Z",
      name: "tom@kestroll.com",
      email: "tom@kestroll.com",
      phone: "",
      org_id: "org-existing",
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

    const result = applyExtractedContactToHuman(
      store,
      "session-1",
      "human-1",
      [
        {
          name: "Tom Yang",
          email: "tom@kestroll.com",
          companyName: "Kestroll",
        },
      ],
      { userId: "user-1" },
    );

    expect(result).toMatchObject({
      updated: 1,
      matched: true,
    });
    expect(store.getCell("humans", "human-1", "name")).toBe("Tom Yang");
    expect(store.getCell("humans", "human-1", "org_id")).toBe("org-existing");
    expect(Object.keys(store.getTable("organizations"))).toHaveLength(1);
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
