import { describe, expect, it } from "vitest";

import { CONTEXT_TEXT_FIELD } from "./context-text";
import { stripEphemeralToolContext } from "./strip-ephemeral-tool-context";

describe("stripEphemeralToolContext", () => {
  it("removes contextText from any completed tool output", () => {
    const parts = [
      {
        type: "tool-read_current_note",
        state: "output-available",
        output: {
          status: "ok",
          title: "Note",
          [CONTEXT_TEXT_FIELD]: "full note content",
        },
      },
    ] as any;

    expect(stripEphemeralToolContext(parts)).toEqual([
      {
        type: "tool-read_current_note",
        state: "output-available",
        output: {
          status: "ok",
          title: "Note",
        },
      },
    ]);
  });
});
