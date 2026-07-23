import type { AnchoredSharedNoteComment } from "@/lib/shared-note-comment-anchors";

export type SharedNoteCommentThread = {
  id: string;
  comments: AnchoredSharedNoteComment[];
};

export function groupSharedNoteCommentThreads(
  comments: readonly AnchoredSharedNoteComment[],
) {
  const threads: SharedNoteCommentThread[] = [];
  const byAnchor = new Map<string, SharedNoteCommentThread>();

  const chronologicalComments = [...comments].sort(
    (left, right) => Date.parse(left.createdAt) - Date.parse(right.createdAt),
  );

  for (const comment of chronologicalComments) {
    const key = anchorKey(comment);
    const existing = byAnchor.get(key);
    if (existing) {
      existing.comments.push(comment);
      continue;
    }
    const thread = { id: comment.commentId, comments: [comment] };
    byAnchor.set(key, thread);
    threads.push(thread);
  }

  return threads;
}

export function getSharedNoteCommentThreadAnchors(
  comments: readonly AnchoredSharedNoteComment[],
) {
  return groupSharedNoteCommentThreads(comments).flatMap((thread) => {
    const range = thread.comments[0]?.range;
    return range ? [{ commentId: thread.id, ...range }] : [];
  });
}

function anchorKey(comment: AnchoredSharedNoteComment) {
  if (!comment.anchor || !comment.range) return `comment:${comment.commentId}`;
  return JSON.stringify([
    comment.anchor.quoteExact,
    comment.anchor.quotePrefix,
    comment.anchor.quoteSuffix,
    comment.range.from,
    comment.range.to,
  ]);
}
