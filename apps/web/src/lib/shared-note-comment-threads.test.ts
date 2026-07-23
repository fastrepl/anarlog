import assert from "node:assert/strict";
import test from "node:test";

import type { AnchoredSharedNoteComment } from "./shared-note-comment-anchors.ts";
import { groupSharedNoteCommentThreads } from "./shared-note-comment-threads.ts";

function comment(commentId: string, from: number): AnchoredSharedNoteComment {
  return {
    commentId,
    isAuthor: false,
    body: commentId,
    snapshotRevision: 1,
    anchor: {
      quoteExact: "same quote",
      quotePrefix: "",
      quoteSuffix: "",
      fromHint: from,
      toHint: from + 10,
    },
    range: { from, to: from + 10 },
    createdAt: "2026-07-23T00:00:00Z",
  };
}

test("groups comments on the same anchored text into one visual thread", () => {
  const threads = groupSharedNoteCommentThreads([
    comment("root", 10),
    comment("reply", 10),
    comment("other", 40),
  ]);

  assert.deepEqual(
    threads.map((thread) => thread.comments.map(({ commentId }) => commentId)),
    [["root", "reply"], ["other"]],
  );
});

test("keeps unresolved comments in separate threads", () => {
  const unresolved = { ...comment("first", 10), range: null };
  assert.equal(
    groupSharedNoteCommentThreads([
      unresolved,
      { ...unresolved, commentId: "second" },
    ]).length,
    2,
  );
});
