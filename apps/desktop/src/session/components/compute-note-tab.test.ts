import { describe, expect, test } from "vitest";

import { computeCurrentNoteTab } from "./compute-note-tab";

describe("computeCurrentNoteTab", () => {
  test("falls back to raw when transcript view is no longer available", () => {
    expect(
      computeCurrentNoteTab({ type: "transcript" }, false, false, []),
    ).toEqual({ type: "raw" });
  });

  test("falls back to the first enhanced note when the selected one is gone", () => {
    expect(
      computeCurrentNoteTab(
        { type: "enhanced", id: "missing-note" },
        false,
        true,
        ["note-1"],
      ),
    ).toEqual({ type: "enhanced", id: "note-1" });
  });

  test("keeps transcript view while the listener is active", () => {
    expect(
      computeCurrentNoteTab({ type: "transcript" }, true, false, []),
    ).toEqual({ type: "transcript" });
  });
});
