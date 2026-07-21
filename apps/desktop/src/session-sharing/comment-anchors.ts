import { useQuery } from "@tanstack/react-query";
import type { EditorView } from "prosemirror-view";
import { useCallback, useEffect, useState } from "react";

import { resolveCommentAnchors } from "@hypr/editor/comments";
import { setCommentAnchors } from "@hypr/editor/note";

import {
  listSessionShareComments,
  type SessionShareComment,
  ShareManagementError,
} from "./client";

import { useAuth } from "~/auth";
import { loadManagedSharedNoteForSession } from "~/shared-notes/cache";

export function sessionShareCommentsQueryKey(shareId: string) {
  return ["shared-note-comments", shareId] as const;
}

export function applySessionShareCommentAnchors(
  view: EditorView,
  comments: readonly SessionShareComment[],
  currentRevision: number,
) {
  const resolved = resolveCommentAnchors(
    view.state.doc,
    comments.map((comment) => ({
      commentId: comment.commentId,
      anchor: comment.anchor
        ? {
            ...comment.anchor,
            snapshotRevision: comment.snapshotContentRevision,
          }
        : null,
    })),
    currentRevision,
  );
  setCommentAnchors(
    view,
    resolved.flatMap((entry) =>
      entry.range
        ? [
            {
              commentId: entry.commentId,
              from: entry.range.from,
              to: entry.range.to,
            },
          ]
        : [],
    ),
  );
}

/**
 * Display-only comment highlights for the owner's live session editors.
 * Enabled only when the session has a managed share and an authenticated
 * Supabase session; loads the first comment page (v1: no pagination).
 */
export function useSessionCommentAnchors(sessionId: string) {
  const auth = useAuth();
  const [view, setView] = useState<EditorView | null>(null);
  const session =
    auth.session && auth.session.user.is_anonymous !== true
      ? auth.session
      : null;
  const supabase = auth.supabase;
  const userId = session?.user.id ?? null;

  const shareQuery = useQuery({
    queryKey: ["session-managed-share", userId ?? "", sessionId],
    queryFn: () => loadManagedSharedNoteForSession(userId!, sessionId),
    enabled: userId !== null,
  });
  const shareId = shareQuery.data?.shareId ?? null;

  const commentsQuery = useQuery({
    queryKey: sessionShareCommentsQueryKey(shareId ?? ""),
    queryFn: ({ signal }) => {
      if (!supabase || !session || !shareId) {
        throw new ShareManagementError();
      }
      return listSessionShareComments(
        { supabase, session, signal },
        { shareId },
      );
    },
    enabled: shareId !== null && session !== null && supabase !== null,
  });
  const comments = commentsQuery.data?.comments;

  // External sync: highlights live in the ProseMirror comment-anchors plugin,
  // so resolved ranges are pushed into the view when comments arrive. The
  // live doc has no snapshot revision, so currentRevision -1 keeps the hint
  // fast path off and anchors always re-resolve by quote search; the plugin
  // then maps decorations through subsequent edits.
  useEffect(() => {
    if (!view || !comments) return;
    applySessionShareCommentAnchors(view, comments, -1);
  }, [view, comments]);

  return {
    onViewReady: useCallback((readyView: EditorView) => setView(readyView), []),
    onViewDisposed: useCallback(
      (disposedView: EditorView) =>
        setView((current) => (current === disposedView ? null : current)),
      [],
    ),
  };
}
