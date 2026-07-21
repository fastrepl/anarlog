import { Plugin, PluginKey, type EditorState } from "prosemirror-state";
import { Decoration, DecorationSet, type EditorView } from "prosemirror-view";

export type CommentAnchorInput = {
  commentId: string;
  from: number;
  to: number;
};

export type CommentAnchorsEvent =
  | { type: "selection"; from: number; to: number; empty: boolean }
  | { type: "anchor-click"; commentIds: string[]; pos: number }
  | { type: "layout" };

type CommentAnchorsState = {
  decorations: DecorationSet;
  // Canonical ranges mapped through every transaction; decorations are a
  // render artifact and may fragment, so ranges are never derived from them.
  anchors: CommentAnchorInput[];
  activeId: string | null;
};

type CommentAnchorsMeta =
  | { type: "set"; anchors: CommentAnchorInput[] }
  | { type: "active"; commentId: string | null };

export const commentAnchorsPluginKey = new PluginKey<CommentAnchorsState>(
  "commentAnchors",
);

/**
 * Highlights comment-anchored ranges. Anchors and the active highlight are
 * controlled after mount through transaction metadata (the editor never
 * reconfigures plugins), via setCommentAnchors / setActiveCommentAnchor.
 */
export function commentAnchorsPlugin(options?: {
  onEvent?: (event: CommentAnchorsEvent) => void;
}): Plugin<CommentAnchorsState> {
  const emit = (event: CommentAnchorsEvent) => options?.onEvent?.(event);

  return new Plugin<CommentAnchorsState>({
    key: commentAnchorsPluginKey,
    state: {
      init() {
        return {
          decorations: DecorationSet.empty,
          anchors: [],
          activeId: null,
        };
      },
      apply(tr, pluginState) {
        const meta = tr.getMeta(commentAnchorsPluginKey) as
          | CommentAnchorsMeta
          | undefined;
        if (meta?.type === "set") {
          const anchors = clampAnchors(meta.anchors, tr.doc.content.size);
          return {
            decorations: buildDecorations(
              tr.doc,
              anchors,
              pluginState.activeId,
            ),
            anchors,
            activeId: pluginState.activeId,
          };
        }
        if (meta?.type === "active") {
          const anchors = tr.docChanged
            ? mapAnchorsThrough(tr.mapping, pluginState.anchors)
            : pluginState.anchors;
          return {
            decorations: buildDecorations(tr.doc, anchors, meta.commentId),
            anchors,
            activeId: meta.commentId,
          };
        }
        if (!tr.docChanged) {
          return pluginState;
        }
        return {
          decorations: pluginState.decorations.map(tr.mapping, tr.doc),
          anchors: mapAnchorsThrough(tr.mapping, pluginState.anchors),
          activeId: pluginState.activeId,
        };
      },
    },
    props: {
      decorations(state) {
        return commentAnchorsPluginKey.getState(state)?.decorations;
      },
      handleClick(view, pos) {
        const pluginState = commentAnchorsPluginKey.getState(view.state);
        if (!pluginState) return false;
        const hits = pluginState.decorations.find(pos, pos);
        const commentIds = [
          ...new Set(
            hits
              .map((decoration) => decoration.spec.commentId as string)
              .filter(Boolean),
          ),
        ];
        if (commentIds.length > 0) {
          emit({ type: "anchor-click", commentIds, pos });
        }
        return false;
      },
    },
    view() {
      return {
        update(view, prevState) {
          const selection = view.state.selection;
          if (!selection.eq(prevState.selection)) {
            emit({
              type: "selection",
              from: selection.from,
              to: selection.to,
              empty: selection.empty,
            });
          }
          const decorations = commentAnchorsPluginKey.getState(
            view.state,
          )?.decorations;
          const previousDecorations =
            commentAnchorsPluginKey.getState(prevState)?.decorations;
          if (
            view.state.doc !== prevState.doc ||
            decorations !== previousDecorations
          ) {
            emit({ type: "layout" });
          }
        },
      };
    },
  });
}

export function setCommentAnchors(
  view: EditorView,
  anchors: CommentAnchorInput[],
): void {
  view.dispatch(
    view.state.tr.setMeta(commentAnchorsPluginKey, {
      type: "set",
      anchors,
    } satisfies CommentAnchorsMeta),
  );
}

export function setActiveCommentAnchor(
  view: EditorView,
  commentId: string | null,
): void {
  view.dispatch(
    view.state.tr.setMeta(commentAnchorsPluginKey, {
      type: "active",
      commentId,
    } satisfies CommentAnchorsMeta),
  );
}

/** Current anchor ranges, mapped through any edits since they were set. */
export function getCommentAnchorRanges(
  state: EditorState,
): CommentAnchorInput[] {
  return commentAnchorsPluginKey.getState(state)?.anchors ?? [];
}

/** Viewport coordinates for each anchor, for aligning side cards. */
export function getCommentAnchorScreenPositions(
  view: EditorView,
): Array<CommentAnchorInput & { top: number; bottom: number; left: number }> {
  return getCommentAnchorRanges(view.state).map((anchor) => {
    const start = view.coordsAtPos(anchor.from);
    const end = view.coordsAtPos(anchor.to);
    return {
      ...anchor,
      top: Math.min(start.top, end.top),
      bottom: Math.max(start.bottom, end.bottom),
      left: start.left,
    };
  });
}

/** Viewport rectangle spanning the current selection, for floating UI. */
export function getSelectionScreenRect(
  view: EditorView,
): { left: number; top: number; right: number; bottom: number } | null {
  const { from, to, empty } = view.state.selection;
  if (empty) return null;
  const start = view.coordsAtPos(from);
  const end = view.coordsAtPos(to);
  return {
    left: Math.min(start.left, end.left),
    top: Math.min(start.top, end.top),
    right: Math.max(start.right, end.right),
    bottom: Math.max(start.bottom, end.bottom),
  };
}

function mapAnchorsThrough(
  mapping: { map(pos: number, assoc?: number): number },
  anchors: CommentAnchorInput[],
): CommentAnchorInput[] {
  return anchors.flatMap((anchor) => {
    const from = mapping.map(anchor.from, 1);
    const to = mapping.map(anchor.to, -1);
    return from < to ? [{ ...anchor, from, to }] : [];
  });
}

function buildDecorations(
  doc: Parameters<typeof DecorationSet.create>[0],
  anchors: CommentAnchorInput[],
  activeId: string | null,
): DecorationSet {
  const size = doc.content.size;
  const decorations = anchors
    .filter((anchor) => anchor.from < anchor.to && anchor.to <= size)
    .map((anchor) =>
      Decoration.inline(
        anchor.from,
        anchor.to,
        {
          class:
            anchor.commentId === activeId
              ? "comment-anchor comment-anchor-active"
              : "comment-anchor",
          "data-comment-id": anchor.commentId,
        },
        { commentId: anchor.commentId },
      ),
    );
  return DecorationSet.create(doc, decorations);
}

function clampAnchors(
  anchors: CommentAnchorInput[],
  size: number,
): CommentAnchorInput[] {
  return anchors.filter(
    (anchor) =>
      anchor.from < anchor.to && anchor.from >= 0 && anchor.to <= size,
  );
}
