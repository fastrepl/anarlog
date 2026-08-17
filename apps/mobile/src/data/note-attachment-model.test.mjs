import assert from "node:assert/strict";
import test from "node:test";

import {
  insertNoteAttachmentMarkdown,
  portableNoteAttachmentMarkdown,
  requireNoteAttachmentFilename,
} from "./note-attachment-model.ts";

const attachmentId = "9cfb2f08-a02f-41cf-a13e-07f36b87ef2b";

test("builds a portable attachment link with a safe label", () => {
  assert.equal(
    portableNoteAttachmentMarkdown(attachmentId, "plan [final].pdf"),
    `[plan (final).pdf](attachment://${attachmentId})`,
  );
});

test("rejects path-like attachment names", () => {
  for (const filename of ["", "../plan.pdf", "folder/plan.pdf", "bad\nname"]) {
    assert.throws(() => requireNoteAttachmentFilename(filename));
  }
});

test("inserts the attachment on its own Markdown line", () => {
  const markdown = portableNoteAttachmentMarkdown(attachmentId, "plan.pdf");
  assert.deepEqual(
    insertNoteAttachmentMarkdown("beforeafter", { start: 6, end: 6 }, markdown),
    {
      text: `before\n${markdown}\nafter`,
      selection: {
        start: `before\n${markdown}\n`.length,
        end: `before\n${markdown}\n`.length,
      },
    },
  );
});
