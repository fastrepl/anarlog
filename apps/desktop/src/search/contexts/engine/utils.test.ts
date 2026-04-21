import { describe, expect, test } from "vitest";

import { extractPlainText, getSessionSearchTimestamp } from "./utils";

describe("getSessionSearchTimestamp", () => {
  test("prefers embedded event started_at over session created_at", () => {
    expect(
      getSessionSearchTimestamp({
        created_at: "2024-01-01T00:00:00Z",
        event_json: JSON.stringify({
          started_at: "2024-01-15T10:00:00Z",
        }),
      }),
    ).toBe(Date.parse("2024-01-15T10:00:00Z"));
  });

  test("falls back to session created_at when event started_at is missing", () => {
    expect(
      getSessionSearchTimestamp({
        created_at: "2024-01-01T00:00:00Z",
        event_json: JSON.stringify({
          title: "Planning",
        }),
      }),
    ).toBe(Date.parse("2024-01-01T00:00:00Z"));
  });
});

describe("extractPlainText", () => {
  test("drops malformed nested tiptap nodes while preserving text", () => {
    expect(
      extractPlainText(
        JSON.stringify({
          type: "doc",
          content: [
            null,
            {
              type: "paragraph",
              content: [
                { type: "text", text: "hello" },
                undefined,
                { text: "ignored" },
                {
                  type: "text",
                  text: "world",
                },
              ],
            },
          ],
        }),
      ),
    ).toBe("hello world");
  });
});
