import { describe, expect, test } from "vitest";

import { createSessionSearchableContent } from "./content";

describe("createSessionSearchableContent", () => {
  test("includes source app names and ids", () => {
    const content = createSessionSearchableContent({
      source_app_json: JSON.stringify([
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
      ]),
      raw_md: "sales follow-up",
      enhanced_notes_content: "",
      transcript: "",
    });

    expect(content).toContain("Slack");
    expect(content).toContain("com.tinyspeck.slackmacgap");
  });
});
