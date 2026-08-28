import { CircleNotch, File, Image } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import {
  type ComponentProps,
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { colors, fonts } from "@anlg/design-system/tokens.stylex";
import {
  captureCommentAnchor,
  type CommentAnchor,
} from "@anlg/editor/comments";
import {
  type CommentAnchorsEvent,
  getCommentAnchorScreenPositions,
  getSelectionScreenRect,
  NoteEditor,
  type NoteEditorProps,
  type NoteEditorRef,
  schema,
  setActiveCommentAnchor,
  setCommentAnchors,
} from "@anlg/editor/note";
import { Avatar } from "@anlg/ui/components/avatar";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import {
  DRAFT_COMMENT_ID,
  SharedNoteCommentCard,
  SharedNoteCommentRail,
} from "@/components/shared-note-comment-rail";
import {
  collectSharedNoteComments,
  useCreateSharedNoteComment,
  useDeleteSharedNoteComment,
  useSharedNoteComments,
} from "@/components/shared-note-comments-data";
import {
  type SharedAttachmentResolver,
  SharedNoteDocument,
} from "@/components/shared-note-document";
import {
  type SelectionRect,
  SharedNoteSelectionComment,
} from "@/components/shared-note-selection-comment";
import { sharedButtonStyles } from "@/components/shared-note-viewer";
import {
  hasSharedNoteCollaborationAccess,
  MAX_SHARED_NOTE_COMMENT_BYTES,
  validateSharedNoteCommentBody,
} from "@/lib/shared-note-collaboration";
import {
  type AnchoredSharedNoteComment,
  fromCaptured,
  resolveSharedNoteCommentRanges,
} from "@/lib/shared-note-comment-anchors";
import { pickActiveCommentId } from "@/lib/shared-note-comment-rail-layout";
import {
  getSharedNoteCommentThreadAnchors,
  groupSharedNoteCommentThreads,
} from "@/lib/shared-note-comment-threads";
import {
  isMatchingSharedNoteAttachmentDownload,
  type SharedNoteAttachment,
  type SharedNoteNode,
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style1: {
    position: "relative",
  },
  style2: {
    outlineStyle: "none",
    outlineOffset: {
      default: null,
      "@media (forced-colors: active)": "2px",
    },
    outline: {
      default: null,
      "@media (forced-colors: active)": "2px solid #0000",
    },
  },
  style3: {
    marginTop: "2.5rem",
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingTop: "1.5rem",
  },
  style4: {
    marginBottom: ".75rem",
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style5: {
    position: "absolute",
    insetBlock: 0,
    left: "100%",
    marginLeft: "1.5rem",
    display: {
      default: "none",
      "@media (width >= 80rem)": "block",
    },
    width: "284px",
  },
  style6: {
    position: "fixed",
    insetInline: "1rem",
    bottom: "6rem",
    zIndex: 40,
    marginInline: "auto",
    width: "auto",
    display: {
      default: null,
      "@media (width >= 80rem)": "none",
    },
  },
  style7: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1rem",
    boxShadow: "0 4px 6px -1px #0000001a, 0 2px 4px -2px #0000001a",
  },
  style8: {
    display: "flex",
    alignItems: "flex-start",
    gap: ".625rem",
  },
  style9: {
    marginTop: ".25rem",
  },
  style10: {
    marginTop: ".5rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#b91c1c",
  },
  style11: {
    marginTop: ".75rem",
    display: "flex",
    justifyContent: "flex-end",
    gap: ".5rem",
  },
  style12: {
    marginRight: ".375rem",
    width: ".875rem",
    height: ".875rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: spin,
  },
  style13: {
    marginBlock: "1.5rem",
  },
  style14: {
    maxHeight: "70vh",
    maxWidth: "100%",
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    objectFit: "contain",
  },
  style15: {
    marginBlock: "1rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
    color: colors.foreground,
    textDecorationLine: "none",
  },
  style16: {
    minWidth: 0,
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontWeight: 500,
  },
  style17: {
    flexShrink: 0,
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  style18: {
    marginBlock: ".75rem",
    display: "flex",
    alignItems: "center",
    gap: ".75rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
  },
  style19: {
    display: "flex",
    width: "2.5rem",
    height: "2.5rem",
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: colors.card,
    borderRadius: ".5rem",
  },
  style20: {
    color: colors.mutedForeground,
    width: "1.25rem",
    height: "1.25rem",
  },
  style21: {
    minWidth: 0,
    flexGrow: 1,
  },
  style22: {
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: colors.foreground,
  },
  style23: {
    marginTop: ".125rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  commentTextarea: {
    backgroundColor: colors.muted,
    borderColor: {
      default: colors.border,
      ":focus": "#a8a29e",
    },
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus": "0 0 0 2px #d6d3d1",
    },
    color: colors.foreground,
    flexGrow: 1,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    minHeight: "5rem",
    minWidth: 0,
    outline: {
      default: null,
      ":focus": "none",
    },
    paddingBlock: ".5rem",
    paddingInline: ".75rem",
    resize: "vertical",
    "::placeholder": {
      color: colors.mutedForeground,
    },
  },
  compactCommentButton: {
    fontSize: ".75rem",
    minHeight: "2.25rem",
    paddingInline: ".75rem",
  },
});
type EditorView = NonNullable<NoteEditorRef["view"]>;
type EditorNodeView = NonNullable<NoteEditorProps["extraNodeViews"]>[string];
type EditorNodeViewProps = ComponentProps<EditorNodeView>;
const SharedReadAttachmentsContext = createContext<{
  attachments: ReadonlyMap<string, SharedNoteAttachment>;
  excluded: ReadonlySet<string>;
  resolve: SharedAttachmentResolver | null;
}>({
  attachments: new Map(),
  excluded: new Set(),
  resolve: null,
});

// Keep JS visibility in lockstep with the comment rail layout breakpoint.
const RAIL_MEDIA_QUERY = "(min-width: 80rem)";
function subscribeRailMedia(onChange: () => void) {
  const media = window.matchMedia(RAIL_MEDIA_QUERY);
  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}
function useCommentRailVisible() {
  return useSyncExternalStore(
    subscribeRailMedia,
    () => window.matchMedia(RAIL_MEDIA_QUERY).matches,
    () => false,
  );
}
export function SharedNoteReadSurface({
  canCompose,
  excludedAttachmentIds = [],
  manageAccess,
  resolveAttachment,
  shareId,
  signedIn,
  snapshot,
}: {
  canCompose: boolean;
  excludedAttachmentIds?: readonly string[];
  manageAccess: boolean;
  resolveAttachment?: SharedAttachmentResolver;
  shareId: string;
  signedIn: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const [view, setView] = useState<EditorView | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const frameRef = useRef<number | null>(null);
  const [screenTops, setScreenTops] = useState<ReadonlyMap<string, number>>(
    new Map(),
  );
  const [activeCommentId, setActiveCommentId] = useState<string | null>(null);
  const [selectionRect, setSelectionRect] = useState<SelectionRect | null>(
    null,
  );
  const [draft, setDraft] = useState<{
    anchor: CommentAnchor;
    from: number;
    to: number;
    top: number;
  } | null>(null);
  const [anchoredComments, setAnchoredComments] = useState<
    AnchoredSharedNoteComment[]
  >([]);
  const railVisible = useCommentRailVisible();
  const commentsQuery = useSharedNoteComments({
    enabled: signedIn,
    shareId,
  });
  const createMutation = useCreateSharedNoteComment({
    shareId,
    snapshotRevision: snapshot.contentRevision,
  });
  const deleteMutation = useDeleteSharedNoteComment({
    shareId,
  });
  const comments = useMemo(
    () => collectSharedNoteComments(commentsQuery.data),
    [commentsQuery.data],
  );
  const composeEnabled =
    canCompose &&
    hasSharedNoteCollaborationAccess(commentsQuery.data?.pages[0]);
  const railItems = anchoredComments.filter(
    (comment) => comment.anchor !== null && comment.range !== null,
  );
  const commentThreads = groupSharedNoteCommentThreads(railItems);
  const activeThread = activeCommentId
    ? (commentThreads.find((thread) =>
        thread.comments.some(
          (comment) => comment.commentId === activeCommentId,
        ),
      ) ?? null)
    : null;
  const activeComment = activeThread?.comments[0] ?? null;
  const railHasContent = signedIn && (draft !== null || railItems.length > 0);
  const body = useMemo(
    () => withoutDuplicateLeadingTitle(snapshot.body, snapshot.title),
    [snapshot],
  );
  // The editor silently falls back to an empty document for content the
  // schema rejects, so anything unparsable keeps the static renderer.
  const editorBodyIsValid = useMemo(() => {
    try {
      schema.nodeFromJSON(body).check();
      return true;
    } catch {
      return false;
    }
  }, [body]);
  const attachmentContext = useMemo(
    () => ({
      attachments: new Map(
        snapshot.attachments.map((attachment) => [attachment.id, attachment]),
      ),
      excluded: new Set(excludedAttachmentIds),
      resolve: resolveAttachment ?? null,
    }),
    [snapshot.attachments, excludedAttachmentIds, resolveAttachment],
  );
  const unreferencedAttachments = useMemo(() => {
    const referenced = new Set<string>();
    const visit = (node: SharedNoteNode) => {
      if (typeof node.attrs?.sharedAttachmentId === "string") {
        referenced.add(node.attrs.sharedAttachmentId);
      }
      node.content?.forEach(visit);
    };
    visit(body);
    return snapshot.attachments.filter(
      (attachment) =>
        !referenced.has(attachment.id) &&
        !excludedAttachmentIds.includes(attachment.id),
    );
  }, [body, excludedAttachmentIds, snapshot.attachments]);
  const scheduleLayoutMeasure = useCallback(() => {
    if (frameRef.current !== null) return;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = null;
      const currentView = viewRef.current;
      const container = containerRef.current;
      if (!currentView || !container) return;
      const containerTop = container.getBoundingClientRect().top;
      const positions = getCommentAnchorScreenPositions(currentView);
      setScreenTops((previous) => {
        const next = new Map(
          positions.map((position) => [
            position.commentId,
            position.top - containerTop,
          ]),
        );
        const unchanged =
          previous.size === next.size &&
          [...next].every(([id, top]) => previous.get(id) === top);
        return unchanged ? previous : next;
      });
    });
  }, []);
  const attachContainer = useCallback(
    (element: HTMLDivElement | null) => {
      if (!element) return;
      containerRef.current = element;
      const observer = new ResizeObserver(() => scheduleLayoutMeasure());
      observer.observe(element);
      return () => {
        observer.disconnect();
        containerRef.current = null;
        if (frameRef.current !== null) {
          cancelAnimationFrame(frameRef.current);
          frameRef.current = null;
        }
      };
    },
    [scheduleLayoutMeasure],
  );

  // External sync: anchor highlights live in the ProseMirror plugin, so
  // resolved ranges are pushed into the view whenever comments change.
  useEffect(() => {
    if (!view) return;
    const anchored = resolveSharedNoteCommentRanges(
      view.state.doc,
      comments,
      snapshot.contentRevision,
    );
    setAnchoredComments(anchored);
    setCommentAnchors(view, [
      ...getSharedNoteCommentThreadAnchors(anchored),
      ...(draft
        ? [
            {
              commentId: DRAFT_COMMENT_ID,
              from: draft.from,
              to: draft.to,
            },
          ]
        : []),
    ]);
    scheduleLayoutMeasure();
  }, [view, comments, draft, snapshot.contentRevision, scheduleLayoutMeasure]);
  const activateComment = (commentId: string | null) => {
    setActiveCommentId(commentId);
    const currentView = viewRef.current;
    if (currentView) setActiveCommentAnchor(currentView, commentId);
  };
  const handleAnchorsEvent = (event: CommentAnchorsEvent) => {
    const currentView = viewRef.current;
    if (!currentView) return;
    if (event.type === "selection") {
      if (!composeEnabled || draft) {
        setSelectionRect(null);
        return;
      }
      // Only offer the pill for selections a draft can actually anchor to;
      // startDraft silently no-ops when anchor capture fails.
      const anchorable =
        !event.empty &&
        captureCommentAnchor(
          currentView.state.doc,
          event.from,
          event.to,
          snapshot.contentRevision,
        ) !== null;
      setSelectionRect(anchorable ? getSelectionScreenRect(currentView) : null);
      return;
    }
    if (event.type === "anchor-click") {
      const picked = pickActiveCommentId(
        getCommentAnchorScreenPositions(currentView),
        event.commentIds.filter((id) => id !== DRAFT_COMMENT_ID),
      );
      if (picked) activateComment(picked);
      return;
    }
    scheduleLayoutMeasure();
  };
  const startDraft = () => {
    const currentView = viewRef.current;
    const container = containerRef.current;
    if (!currentView || !container) return;
    const { from, to } = currentView.state.selection;
    const captured = captureCommentAnchor(
      currentView.state.doc,
      from,
      to,
      snapshot.contentRevision,
    );
    if (!captured) return;
    const top =
      currentView.coordsAtPos(from).top - container.getBoundingClientRect().top;
    // A previous draft's failed submit must not surface its error in the
    // composer of this new draft.
    createMutation.reset();
    setSelectionRect(null);
    setActiveCommentId(null);
    setActiveCommentAnchor(currentView, null);
    setDraft({
      anchor: captured,
      from,
      to,
      top,
    });
  };
  const submitDraft = (commentBody: string) => {
    if (!draft) return;
    const submitted = draft;
    createMutation.mutate(
      {
        anchor: fromCaptured(submitted.anchor),
        body: commentBody,
      },
      {
        // Only clear the draft this submit belongs to; a draft opened after
        // a resize-triggered cleanup must survive the earlier completion.
        onSuccess: () =>
          setDraft((current) => (current === submitted ? null : current)),
      },
    );
  };
  if (!editorBodyIsValid) {
    return (
      <SharedNoteDocument
        attachments={snapshot.attachments}
        document={body}
        excludedAttachmentIds={excludedAttachmentIds}
        resolveAttachment={resolveAttachment}
      />
    );
  }
  return (
    <div
      ref={attachContainer}
      {...stylex.props(styles.style1)}
      data-comment-rail={railHasContent ? "" : undefined}
    >
      <SharedReadAttachmentsContext.Provider value={attachmentContext}>
        <NoteEditor
          {...mergeStyleXProps(styles.style2, "session-note-editor")}
          commentAnchorsEnabled
          enforceTitleHeading={false}
          extraNodeViews={readAttachmentNodeViews}
          initialContent={body}
          onCommentAnchorsEvent={handleAnchorsEvent}
          onViewDisposed={() => {
            viewRef.current = null;
            setView(null);
          }}
          onViewReady={(readyView) => {
            viewRef.current = readyView;
            setView(readyView);
          }}
          readOnly
          showFormatToolbar={false}
          showSlashCommand={false}
        />
      </SharedReadAttachmentsContext.Provider>
      {unreferencedAttachments.length > 0 && (
        <section {...stylex.props(styles.style3)}>
          <h2 {...stylex.props(styles.style4)}>Attachments</h2>
          {unreferencedAttachments.map((attachment) => (
            <SharedReadAttachment
              key={attachment.id}
              attachment={attachment}
              isImage={false}
              resolve={resolveAttachment ?? null}
            />
          ))}
        </section>
      )}
      <SharedNoteSelectionComment
        onStart={startDraft}
        rect={selectionRect}
        visible={composeEnabled && !draft}
      />
      <div {...stylex.props(styles.style5)}>
        <SharedNoteCommentRail
          activeCommentId={activeCommentId}
          canDelete={(comment) => comment.isAuthor || manageAccess}
          composer={
            railVisible && draft
              ? {
                  top: screenTops.get(DRAFT_COMMENT_ID) ?? draft.top,
                }
              : null
          }
          composerNode={
            railVisible && draft ? (
              <DraftComposer
                error={createMutation.isError}
                onCancel={() => {
                  createMutation.reset();
                  setDraft(null);
                }}
                onSubmit={submitDraft}
                pending={createMutation.isPending}
              />
            ) : undefined
          }
          deletePending={deleteMutation.isPending}
          deletingCommentId={deleteMutation.variables ?? null}
          items={railItems}
          onActivate={activateComment}
          onDelete={(commentId) => deleteMutation.mutate(commentId)}
          onReply={
            composeEnabled
              ? (comment, commentBody) =>
                  createMutation.mutateAsync({
                    anchor: comment.anchor,
                    body: commentBody,
                  })
              : undefined
          }
          screenTops={screenTops}
        />
      </div>
      {!railVisible && (draft || activeComment) && (
        <div {...stylex.props(styles.style6)}>
          {draft ? (
            <DraftComposer
              error={createMutation.isError}
              onCancel={() => {
                createMutation.reset();
                setDraft(null);
              }}
              onSubmit={submitDraft}
              pending={createMutation.isPending}
            />
          ) : activeComment ? (
            <SharedNoteCommentCard
              active
              canDelete={(comment) => comment.isAuthor || manageAccess}
              comment={activeComment}
              deleteDisabled={deleteMutation.isPending}
              deletingCommentId={deleteMutation.variables ?? null}
              onActivate={() => activateComment(null)}
              onDelete={(commentId) => deleteMutation.mutate(commentId)}
              onReply={
                composeEnabled
                  ? (comment, commentBody) =>
                      createMutation.mutateAsync({
                        anchor: comment.anchor,
                        body: commentBody,
                      })
                  : undefined
              }
              replies={activeThread?.comments.slice(1)}
            />
          ) : null}
        </div>
      )}
    </div>
  );
}
function DraftComposer({
  error,
  onCancel,
  onSubmit,
  pending,
}: {
  error: boolean;
  onCancel: () => void;
  onSubmit: (body: string) => void;
  pending: boolean;
}) {
  const form = useForm({
    defaultValues: {
      body: "",
    },
    onSubmit: ({ value }) => {
      const comment = validateSharedNoteCommentBody(value.body);
      if (!comment.valid) return;
      onSubmit(comment.body);
    },
  });
  return (
    <form
      {...stylex.props(styles.style7)}
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <form.Field name="body">
        {(field) => {
          const comment = validateSharedNoteCommentBody(field.state.value);
          const tooLong = comment.byteLength > MAX_SHARED_NOTE_COMMENT_BYTES;
          return (
            <>
              <div {...stylex.props(styles.style8)}>
                <Avatar
                  seed="shared-note:you"
                  label="You"
                  size={28}
                  sx={styles.style9}
                />
                <textarea
                  autoFocus
                  aria-label="Comment on selected text"
                  {...stylex.props(styles.commentTextarea)}
                  aria-invalid={tooLong}
                  placeholder="Comment on the selected text…"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                />
              </div>
              {tooLong && (
                <p {...stylex.props(styles.style10)} role="alert">
                  Comment is too long ({comment.byteLength.toLocaleString()}/
                  {MAX_SHARED_NOTE_COMMENT_BYTES.toLocaleString()} bytes).
                </p>
              )}
              {error && (
                <p {...stylex.props(styles.style10)} role="status">
                  Your comment couldn’t be added. Try again.
                </p>
              )}
              <div {...stylex.props(styles.style11)}>
                <button
                  type="button"
                  {...stylex.props(
                    sharedButtonStyles.base,
                    sharedButtonStyles.secondary,
                    styles.compactCommentButton,
                  )}
                  disabled={pending}
                  onClick={onCancel}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  {...stylex.props(
                    sharedButtonStyles.base,
                    sharedButtonStyles.primary,
                    styles.compactCommentButton,
                  )}
                  disabled={pending || !comment.valid}
                >
                  {pending && (
                    <CircleNotch
                      {...stylex.props(styles.style12)}
                      aria-hidden="true"
                    />
                  )}
                  Comment
                </button>
              </div>
            </>
          );
        }}
      </form.Field>
    </form>
  );
}
const SharedReadAttachmentView = forwardRef<
  HTMLDivElement,
  EditorNodeViewProps
>(function SharedReadAttachmentView({ nodeProps, ...htmlAttrs }, ref) {
  const { attachments, excluded, resolve } = useContext(
    SharedReadAttachmentsContext,
  );
  const sharedAttachmentId = nodeProps.node.attrs.sharedAttachmentId;
  const attachment =
    typeof sharedAttachmentId === "string"
      ? attachments.get(sharedAttachmentId)
      : undefined;
  return (
    <div
      ref={ref}
      {...htmlAttrs}
      contentEditable={false}
      suppressContentEditableWarning
    >
      {attachment && excluded.has(attachment.id) ? null : (
        <SharedReadAttachment
          attachment={attachment}
          isImage={nodeProps.node.type.name === "image"}
          resolve={resolve}
        />
      )}
    </div>
  );
});
const readAttachmentNodeViews = {
  clip: SharedReadAttachmentView,
  fileAttachment: SharedReadAttachmentView,
  image: SharedReadAttachmentView,
};
function SharedReadAttachment({
  attachment,
  isImage,
  resolve,
}: {
  attachment: SharedNoteAttachment | undefined;
  isImage: boolean;
  resolve: SharedAttachmentResolver | null;
}) {
  const downloadQuery = useQuery({
    queryKey: ["shared-note-attachment-download", attachment?.id ?? ""],
    queryFn: ({ signal }) => resolve!(attachment!, signal),
    enabled: Boolean(attachment && resolve),
    retry: false,
    staleTime: 45_000,
    refetchInterval: 45_000,
    gcTime: 0,
  });
  const download =
    !downloadQuery.error &&
    attachment &&
    isMatchingSharedNoteAttachmentDownload(attachment, downloadQuery.data)
      ? downloadQuery.data
      : null;
  if (
    attachment &&
    download &&
    isImage &&
    isInlineImage(attachment.contentType)
  ) {
    return (
      <figure {...stylex.props(styles.style13)}>
        <img
          src={download.signedUrl}
          alt={attachment.filename}
          loading="lazy"
          referrerPolicy="no-referrer"
          {...stylex.props(styles.style14)}
        />
      </figure>
    );
  }
  if (attachment && download && !isImage) {
    return (
      <a
        href={download.signedUrl}
        download={attachment.filename}
        target="_blank"
        rel="ugc noopener noreferrer"
        referrerPolicy="no-referrer"
        {...stylex.props(styles.style15)}
      >
        <span {...stylex.props(styles.style16)}>{attachment.filename}</span>
        <span {...stylex.props(styles.style17)}>
          {formatFileSize(attachment.sizeBytes)}
        </span>
      </a>
    );
  }
  const Icon = isImage ? Image : File;
  return (
    <div {...stylex.props(styles.style18)}>
      <div {...stylex.props(styles.style19)}>
        <Icon {...stylex.props(styles.style20)} aria-hidden="true" />
      </div>
      <div {...stylex.props(styles.style21)}>
        <p {...stylex.props(styles.style22)}>
          {attachment?.filename ?? "Attachment unavailable"}
        </p>
        <p {...stylex.props(styles.style23)}>
          {attachment
            ? `${formatFileSize(attachment.sizeBytes)} · Included with shared note`
            : "Included attachment"}
        </p>
      </div>
    </div>
  );
}
function isInlineImage(contentType: string) {
  return [
    "image/avif",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/webp",
  ].includes(contentType);
}
function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
