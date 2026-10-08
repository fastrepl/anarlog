import { describe, expect, it } from "vitest";

import { hasSummaryText } from "./use-summary-quick-actions";

describe("hasSummaryText", () => {
  it("ignores empty editor documents", () => {
    expect(hasSummaryText("")).toBe(false);
    expect(
      hasSummaryText(
        JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
      ),
    ).toBe(false);
    expect(
      hasSummaryText(
        JSON.stringify({
          type: "doc",
          content: [
            { type: "paragraph", content: [{ type: "text", text: "Hi" }] },
          ],
        }),
      ),
    ).toBe(true);
    expect(hasSummaryText("# Plain markdown")).toBe(true);
  });
});
