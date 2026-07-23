import assert from "node:assert/strict";
import test from "node:test";

import type { AnchoredSharedNoteComment } from "./shared-note-comment-anchors.ts";
import {
  getSharedNoteCommentThreadAnchors,
  groupSharedNoteCommentThreads,
} from "./shared-note-comment-threads.ts";

function comment(
  commentId: string,
  from: number,
  createdAt = "2026-07-23T00:00:00Z",
): AnchoredSharedNoteComment {
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
    createdAt,
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

test("orders thread roots and replies chronologically", () => {
  const threads = groupSharedNoteCommentThreads([
    comment("reply", 10, "2026-07-23T00:02:00Z"),
    comment("other", 40, "2026-07-23T00:03:00Z"),
    comment("root", 10, "2026-07-23T00:01:00Z"),
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

test("creates one anchor per comment thread", () => {
  assert.deepEqual(
    getSharedNoteCommentThreadAnchors([
      comment("reply", 10, "2026-07-23T00:02:00Z"),
      comment("root", 10, "2026-07-23T00:01:00Z"),
      comment("other", 40, "2026-07-23T00:03:00Z"),
    ]),
    [
      { commentId: "root", from: 10, to: 20 },
      { commentId: "other", from: 40, to: 50 },
    ],
  );
});
