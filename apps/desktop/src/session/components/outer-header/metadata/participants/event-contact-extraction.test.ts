import { describe, expect, test } from "vitest";

import {
  buildEventContactExtractionContextFromRecords,
  extractEventContacts,
  planExtractedContactToHuman,
} from "./event-contact-extraction";

const sessionEvent = {
  tracking_id: "event-1",
  calendar_id: "calendar-1",
  title: "Alice Kim <> John",
  started_at: "2026-07-10T09:00:00.000Z",
  ended_at: "2026-07-10T10:00:00.000Z",
  is_all_day: false,
  has_recurrence_rules: false,
  description: "Alice Kim from Example",
};

describe("event contact extraction", () => {
  test("builds context from canonical participants and event attendees", () => {
    const context = buildEventContactExtractionContextFromRecords({
      sessionEvent,
      currentUserId: "user-1",
      participants: [
        {
          humanId: "human-1",
          name: "Alice",
          email: "alice@example.com",
          source: "manual",
        },
        {
          humanId: "human-excluded",
          name: "Excluded",
          email: "excluded@example.com",
          source: "excluded",
        },
      ],
      eventParticipants: [
        { name: "Bob", email: "bob@example.com", is_organizer: true },
      ],
    });

    expect(context.candidates).toEqual([
      expect.objectContaining({ humanId: "human-1", name: "Alice" }),
      expect.objectContaining({ name: "Bob", isOrganizer: true }),
    ]);
  });

  test("deduplicates a session participant and matching attendee", () => {
    const context = buildEventContactExtractionContextFromRecords({
      sessionEvent,
      currentUserId: "user-1",
      participants: [
        {
          humanId: "human-1",
          name: "Alice",
          email: "alice@example.com",
          source: "auto",
        },
      ],
      eventParticipants: [
        {
          name: "Alice Kim",
          email: "ALICE@example.com",
          is_organizer: true,
        },
      ],
    });

    expect(context.candidates).toHaveLength(1);
    expect(context.candidates[0]).toMatchObject({
      humanId: "human-1",
      email: "alice@example.com",
      isOrganizer: true,
    });
  });

  test("plans durable fields without mutating storage", () => {
    const { result, changes } = planExtractedContactToHuman({
      humanId: "human-1",
      userId: "user-1",
      human: { name: "Alice Kim", email: "", organizationId: "" },
      currentUser: { name: "John", email: "john@example.com" },
      mappingSource: "manual",
      contacts: [
        {
          name: "Alice Kim",
          email: "alice@example.com",
          companyName: "Example",
        },
      ],
    });

    expect(result).toMatchObject({ matched: true, updated: 1, skipped: 0 });
    expect(changes).toEqual({
      email: "alice@example.com",
      companyName: "Example",
    });
  });

  test("does not update an excluded participant", () => {
    const { result, changes } = planExtractedContactToHuman({
      humanId: "human-1",
      userId: "user-1",
      human: { name: "Alice", email: "", organizationId: "" },
      currentUser: undefined,
      mappingSource: "excluded",
      contacts: [{ name: "Alice", email: "alice@example.com" }],
    });

    expect(result).toMatchObject({ matched: false, skipped: 1 });
    expect(changes).toEqual({});
  });

  test("does not overwrite the current user", () => {
    const { result, changes } = planExtractedContactToHuman({
      humanId: "user-1",
      userId: "user-1",
      human: { name: "John", email: "john@example.com", organizationId: "" },
      currentUser: { name: "John", email: "john@example.com" },
      mappingSource: "manual",
      contacts: [{ name: "John Jeong", email: "john@example.com" }],
    });

    expect(result).toMatchObject({ matched: true, skipped: 1, updated: 0 });
    expect(changes).toEqual({});
  });

  test("extracts contacts locally from event text and canonical candidates", () => {
    const context = buildEventContactExtractionContextFromRecords({
      sessionEvent,
      currentUserId: "user-1",
      participants: [
        {
          humanId: "human-1",
          name: "Alice Kim",
          email: "alice@example.com",
          source: "manual",
        },
        {
          humanId: "user-1",
          name: "John",
          email: "john@example.com",
          source: "manual",
        },
      ],
      eventParticipants: [],
    });
    const result = extractEventContacts({ context });

    expect(result).toEqual({
      source: "local",
      contacts: [
        {
          name: "Alice Kim",
          email: "alice@example.com",
          companyName: "Example",
        },
      ],
    });
  });

  test("creates a contact from the participant when the human is missing", () => {
    const { result, changes } = planExtractedContactToHuman({
      humanId: "human-1",
      userId: "user-1",
      human: undefined,
      currentUser: { name: "John Jeong", email: "john@example.com" },
      mappingSource: "auto",
      participant: {
        name: "marco.bambini@gmail.com",
        email: "marco.bambini@gmail.com",
      },
      contacts: [],
    });

    expect(result).toMatchObject({ matched: true, created: 1, updated: 0 });
    expect(changes).toEqual({
      name: "Marco Bambini",
      email: "marco.bambini@gmail.com",
    });
  });

  test("derives a name from an email-only human when extraction is empty", () => {
    const { result, changes } = planExtractedContactToHuman({
      humanId: "human-1",
      userId: "user-1",
      human: {
        name: "marco.bambini@gmail.com",
        email: "marco.bambini@gmail.com",
        organizationId: "",
      },
      currentUser: { name: "John Jeong", email: "john@example.com" },
      mappingSource: "auto",
      contacts: [],
    });

    expect(result).toMatchObject({ matched: true, created: 0, updated: 1 });
    expect(changes).toEqual({
      name: "Marco Bambini",
    });
  });

  test("infers a company from a work email when creating a contact", () => {
    const extraction = extractEventContacts({
      context: {
        title: "Check-in",
        candidates: [
          {
            name: "John Jeong",
            email: "john@example.com",
            isCurrentUser: true,
          },
          {
            name: "tom@kestroll.com",
            email: "tom@kestroll.com",
          },
        ],
      },
    });
    const { result, changes } = planExtractedContactToHuman({
      humanId: "human-1",
      userId: "user-1",
      human: undefined,
      currentUser: { name: "John Jeong", email: "john@example.com" },
      mappingSource: "auto",
      participant: {
        name: "tom@kestroll.com",
        email: "tom@kestroll.com",
      },
      contacts: extraction.contacts,
    });

    expect(result).toMatchObject({ matched: true, created: 1 });
    expect(changes).toEqual({
      name: "Tom",
      email: "tom@kestroll.com",
      companyName: "Kestroll",
    });
  });

  test("does not treat a shared first name with a different email as the current user", () => {
    const extraction = extractEventContacts({
      context: {
        title: "Intro",
        candidates: [
          {
            name: "John Jeong",
            email: "john@example.com",
            isCurrentUser: true,
          },
          {
            name: "john@other.com",
            email: "john@other.com",
          },
        ],
      },
    });
    const { result, changes } = planExtractedContactToHuman({
      humanId: "human-1",
      userId: "user-1",
      human: undefined,
      currentUser: { name: "John Jeong", email: "john@example.com" },
      mappingSource: "auto",
      participant: {
        name: "john@other.com",
        email: "john@other.com",
      },
      contacts: extraction.contacts,
    });

    expect(extraction.contacts).toEqual([
      {
        name: "John",
        email: "john@other.com",
        companyName: "Other",
      },
    ]);
    expect(result).toMatchObject({ matched: true, created: 1, skipped: 0 });
    expect(changes).toEqual({
      name: "John",
      email: "john@other.com",
      companyName: "Other",
    });
  });

  test("extracts organizer contacts that only have an email", () => {
    const result = extractEventContacts({
      context: {
        title: "SQLite AI",
        description:
          "<p>Marco Bambini is inviting you to a scheduled Zoom meeting.</p>",
        candidates: [
          {
            humanId: "user-1",
            name: "John Jeong",
            email: "john@example.com",
            isCurrentUser: true,
          },
          {
            humanId: "human-1",
            name: "marco.bambini@gmail.com",
            email: "marco.bambini@gmail.com",
            isOrganizer: true,
          },
        ],
      },
    });

    expect(result.contacts).toEqual([
      {
        name: "Marco Bambini",
        email: "marco.bambini@gmail.com",
      },
    ]);
  });
});
