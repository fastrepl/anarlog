import {
  type CommentAnchor,
  type ResolvedAnchorRange,
  resolveCommentAnchors,
} from "@hypr/editor/comments";

import type {
  SharedNoteComment,
  SharedNoteCommentAnchor,
} from "@/lib/shared-notes";

type EditorDoc = Parameters<typeof resolveCommentAnchors>[0];

export type AnchoredSharedNoteComment = SharedNoteComment & {
  range: ResolvedAnchorRange | null;
};

export function toEditorAnchor(
  anchor: SharedNoteCommentAnchor,
  snapshotRevision: number,
): CommentAnchor {
  return { ...anchor, snapshotRevision };
}

export function fromCaptured(captured: CommentAnchor): SharedNoteCommentAnchor {
  return {
    quoteExact: captured.quoteExact,
    quotePrefix: captured.quotePrefix,
    quoteSuffix: captured.quoteSuffix,
    fromHint: captured.fromHint,
    toHint: captured.toHint,
  };
}

export function resolveSharedNoteCommentRanges(
  doc: EditorDoc,
  comments: readonly SharedNoteComment[],
  currentRevision: number,
): AnchoredSharedNoteComment[] {
  const resolved = resolveCommentAnchors(
    doc,
    comments.map((comment) => ({
      commentId: comment.commentId,
      anchor: comment.anchor
        ? toEditorAnchor(comment.anchor, comment.snapshotRevision)
        : null,
    })),
    currentRevision,
  );
  const rangeById = new Map(
    resolved.map((entry) => [entry.commentId, entry.range]),
  );
  return comments.map((comment) => ({
    ...comment,
    range: rangeById.get(comment.commentId) ?? null,
  }));
}
