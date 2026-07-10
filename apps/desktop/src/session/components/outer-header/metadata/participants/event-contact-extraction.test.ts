import { generateText, type LanguageModel } from "ai";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  buildEventContactExtractionContextFromRecords,
  extractEventContacts,
  planExtractedContactToHuman,
} from "./event-contact-extraction";

const mocks = vi.hoisted(() => ({
  renderTemplate: vi.fn(),
}));

vi.mock("ai", () => ({
  generateText: vi.fn(),
}));

vi.mock("@hypr/plugin-template", () => ({
  commands: { render: mocks.renderTemplate },
}));

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
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.renderTemplate.mockImplementation(async (template: unknown) => {
      if (
        template &&
        typeof template === "object" &&
        ("eventContactSystem" in template || "eventContactUser" in template)
      ) {
        return { status: "success", data: "Prompt" };
      }
      return { status: "error", error: "Unexpected template" };
    });
  });

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

  test("normalizes model output against canonical candidates", async () => {
    vi.mocked(generateText).mockResolvedValue({
      text: JSON.stringify({
        contacts: [
          {
            name: "Alice Kim",
            email: "ALICE@example.com",
            companyName: "Example",
          },
        ],
      }),
    } as never);

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
    const result = await extractEventContacts({
      model: {} as LanguageModel,
      context,
    });

    expect(result).toEqual({
      source: "model",
      contacts: [
        {
          name: "Alice Kim",
          email: "alice@example.com",
          companyName: "Example",
        },
      ],
    });
  });
});
