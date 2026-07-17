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
});
