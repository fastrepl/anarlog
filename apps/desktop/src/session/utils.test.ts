import { describe, expect, it } from "vitest";

import { sessionSearchTimestamp } from "./utils";

describe("sessionSearchTimestamp", () => {
  it("prefers event started_at over session created_at", () => {
    expect(
      sessionSearchTimestamp(
        JSON.stringify({ started_at: "2026-07-14T01:02:03.000Z" }),
        "2025-01-01T00:00:00.000Z",
      ),
    ).toBe(Date.parse("2026-07-14T01:02:03.000Z"));
  });

  it("falls back to created_at without a usable event start", () => {
    expect(sessionSearchTimestamp("{}", "2025-01-01T00:00:00.000Z")).toBe(
      Date.parse("2025-01-01T00:00:00.000Z"),
    );
    expect(sessionSearchTimestamp('{"started_at":1234}', "")).toBe(1234);
  });
});
