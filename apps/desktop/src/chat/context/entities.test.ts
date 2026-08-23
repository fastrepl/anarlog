import { describe, expect, it } from "vitest";

import { extractToolContextEntities } from "./entities";

describe("tool context entities", () => {
  it("extracts meetings from current and historical search tool parts", () => {
    const entities = extractToolContextEntities([
      {
        parts: [
          {
            type: "tool-search_meetings",
            state: "output-available",
            output: {
              results: [{ id: "meeting-1", title: "Planning" }],
            },
          },
          {
            type: "tool-search_sessions",
            state: "output-available",
            output: {
              results: [{ id: "meeting-2", title: "Historical planning" }],
            },
          },
          {
            type: "tool-list_meetings",
            state: "output-available",
            output: {
              meetings: [{ id: "meeting-3", title: "Recent planning" }],
              pagination: {},
            },
          },
        ],
      } as any,
    ]);

    expect(entities).toEqual([
      {
        kind: "session",
        key: "session:search:meeting-1",
        source: "tool",
        sessionId: "meeting-1",
        title: "Planning",
      },
      {
        kind: "session",
        key: "session:search:meeting-2",
        source: "tool",
        sessionId: "meeting-2",
        title: "Historical planning",
      },
      {
        kind: "session",
        key: "session:search:meeting-3",
        source: "tool",
        sessionId: "meeting-3",
        title: "Recent planning",
      },
    ]);
  });

  it("extracts contacts, related meetings, and calendar events", () => {
    const entities = extractToolContextEntities([
      {
        parts: [
          {
            type: "tool-search_contacts",
            state: "output-available",
            output: {
              results: [
                {
                  id: "human-1",
                  name: "Ada",
                  email: "ada@example.com",
                  organization: "Acme",
                },
              ],
            },
          },
          {
            type: "tool-find_related_meetings",
            state: "output-available",
            output: {
              results: [{ meeting_id: "meeting-9", title: "Related" }],
            },
          },
          {
            type: "tool-search_meeting_content",
            state: "output-available",
            output: {
              results: [{ meeting_id: "meeting-10", title: "Mentioned" }],
            },
          },
          {
            type: "tool-search_calendar_events",
            state: "output-available",
            output: {
              results: [
                {
                  id: "evt-1",
                  title: "Standup",
                  linkedSessionId: "session-1",
                },
                { id: "evt-2", title: "Interview" },
              ],
            },
          },
        ],
      } as any,
    ]);

    expect(entities).toEqual([
      {
        kind: "human",
        key: "human:search:human-1",
        source: "tool",
        humanId: "human-1",
        name: "Ada",
        email: "ada@example.com",
        organizationName: "Acme",
      },
      {
        kind: "session",
        key: "session:search:meeting-9",
        source: "tool",
        sessionId: "meeting-9",
        title: "Related",
      },
      {
        kind: "session",
        key: "session:search:meeting-10",
        source: "tool",
        sessionId: "meeting-10",
        title: "Mentioned",
      },
      {
        kind: "calendar_event",
        key: "calendar_event:search:evt-1",
        source: "tool",
        eventId: "evt-1",
        linkedSessionId: "session-1",
        title: "Standup",
      },
      {
        kind: "calendar_event",
        key: "calendar_event:search:evt-2",
        source: "tool",
        eventId: "evt-2",
        linkedSessionId: null,
        title: "Interview",
      },
    ]);
  });
});
