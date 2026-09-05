import assert from "node:assert/strict";
import test from "node:test";

import { hasUnsupportedSharedNoteInteractiveNode } from "./shared-note-interactivity.ts";

test("keeps clips out of the interactive shared-note surface", () => {
  assert.equal(
    hasUnsupportedSharedNoteInteractiveNode({
      type: "doc",
      content: [{ type: "paragraph" }],
    }),
    false,
  );
  assert.equal(
    hasUnsupportedSharedNoteInteractiveNode({
      type: "doc",
      content: [
        {
          type: "blockquote",
          content: [{ type: "clip" }],
        },
      ],
    }),
    true,
  );
});
