import { describe, expect, it } from "vitest";

import { mergeLinkedSessionsIntoContent } from "./linked-session-content";

function buildSessionTitle(title: string) {
  return [
    {
      type: "paragraph",
      content: title ? [{ type: "text", text: title }] : undefined,
    },
  ];
}

describe("mergeLinkedSessionsIntoContent", () => {
  it("deduplicates existing session and event-backed content", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [
          {
            type: "session",
            attrs: { sessionId: "session-1" },
            content: buildSessionTitle("Existing session title"),
          },
          {
            type: "event",
            attrs: { eventId: "event-1" },
            content: [{ type: "text", text: "Legacy event title" }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Body content" }],
          },
        ],
      },
      eventIds: ["event-1", "event-2"],
      sessionIds: ["session-1", "session-3"],
      resolveEventSessionId: (eventId) => {
        if (eventId === "event-1") {
          return "session-1";
        }
        if (eventId === "event-2") {
          return "session-2";
        }
        return null;
      },
      getSessionTitle: (sessionId) =>
        ({
          "session-1": "Session 1",
          "session-2": "Session 2",
          "session-3": "Session 3",
        })[sessionId] ?? "",
    });

    expect(result).toEqual({
      type: "doc",
      content: [
        {
          type: "session",
          attrs: { sessionId: "session-1" },
          content: buildSessionTitle("Existing session title"),
        },
        {
          type: "session",
          attrs: { sessionId: "session-2" },
          content: buildSessionTitle("Session 2"),
        },
        {
          type: "session",
          attrs: { sessionId: "session-3" },
          content: buildSessionTitle("Session 3"),
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Body content" }],
        },
      ],
    });
  });

  it("falls back to an empty paragraph when no linked or user content remains", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [{ type: "event", attrs: { eventId: "missing" } }],
      },
      eventIds: [],
      sessionIds: [],
      resolveEventSessionId: () => null,
      getSessionTitle: () => "",
    });

    expect(result).toEqual({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
  });

  it("deduplicates event-backed sessions after normalizing stale session ids", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [
          {
            type: "session",
            attrs: { sessionId: "stale-session-1" },
            content: buildSessionTitle("Existing title"),
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Body content" }],
          },
        ],
      },
      eventIds: ["event-1"],
      sessionIds: ["stale-session-2"],
      resolveEventSessionId: () => "canonical-session",
      getSessionTitle: (sessionId) =>
        ({
          "canonical-session": "Canonical title",
        })[sessionId] ?? "",
      normalizeSessionId: (sessionId) =>
        sessionId.startsWith("stale-") ? "canonical-session" : sessionId,
    });

    expect(result).toEqual({
      type: "doc",
      content: [
        {
          type: "session",
          attrs: { sessionId: "canonical-session" },
          content: buildSessionTitle("Existing title"),
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Body content" }],
        },
      ],
    });
  });

  it("drops stale linked sessions that are no longer part of the canonical day set", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [
          {
            type: "session",
            attrs: { sessionId: "stale-session" },
            content: buildSessionTitle("Stale title"),
          },
          {
            type: "session",
            attrs: { sessionId: "canonical-session" },
            content: buildSessionTitle("Kept title"),
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Body content" }],
          },
        ],
      },
      eventIds: [],
      sessionIds: ["canonical-session"],
      resolveEventSessionId: () => null,
      getSessionTitle: (sessionId) =>
        ({
          "canonical-session": "Canonical title",
          "stale-session": "Stale title",
        })[sessionId] ?? "",
      keepLinkedSession: (sessionId) => sessionId === "canonical-session",
    });

    expect(result).toEqual({
      type: "doc",
      content: [
        {
          type: "session",
          attrs: { sessionId: "canonical-session" },
          content: buildSessionTitle("Kept title"),
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Body content" }],
        },
      ],
    });
  });

  it("preserves an existing linked session checked state", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [
          {
            type: "session",
            attrs: { sessionId: "session-1", status: "done", checked: true },
            content: buildSessionTitle("Existing session title"),
          },
        ],
      },
      eventIds: [],
      sessionIds: ["session-1"],
      resolveEventSessionId: () => null,
      getSessionTitle: () => "Session 1",
    });

    expect(result).toEqual({
      type: "doc",
      content: [
        {
          type: "session",
          attrs: { sessionId: "session-1", status: "done", checked: true },
          content: buildSessionTitle("Existing session title"),
        },
      ],
    });
  });

  it("preserves linked session placement and appends missing linked sessions after the last linked node", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Intro" }],
          },
          {
            type: "session",
            attrs: { sessionId: "session-1" },
            content: buildSessionTitle("Moved title"),
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Body content" }],
          },
        ],
      },
      eventIds: [],
      sessionIds: ["session-1", "session-2"],
      resolveEventSessionId: () => null,
      getSessionTitle: (sessionId) =>
        ({
          "session-1": "Session 1",
          "session-2": "Session 2",
        })[sessionId] ?? "",
    });

    expect(result).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Intro" }],
        },
        {
          type: "session",
          attrs: { sessionId: "session-1" },
          content: buildSessionTitle("Moved title"),
        },
        {
          type: "session",
          attrs: { sessionId: "session-2" },
          content: buildSessionTitle("Session 2"),
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Body content" }],
        },
      ],
    });
  });
});
