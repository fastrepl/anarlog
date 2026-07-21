import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { FileIcon, ImageIcon, LoaderCircleIcon } from "lucide-react";
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

import {
  captureCommentAnchor,
  type CommentAnchor,
} from "@hypr/editor/comments";
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
} from "@hypr/editor/note";
import { cn } from "@hypr/utils";

import {
  DRAFT_COMMENT_ID,
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
import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
} from "@/components/shared-note-viewer";
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
  type SharedNoteAttachment,
  type SharedNoteAttachmentDownload,
  type SharedNoteNode,
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

type EditorView = NonNullable<NoteEditorRef["view"]>;
type EditorNodeView = NonNullable<NoteEditorProps["extraNodeViews"]>[string];
type EditorNodeViewProps = ComponentProps<EditorNodeView>;

const SharedReadAttachmentsContext = createContext<{
  attachments: ReadonlyMap<string, SharedNoteAttachment>;
  resolve: SharedAttachmentResolver | null;
}>({ attachments: new Map(), resolve: null });

// Tailwind's xl breakpoint — the width from which the comment rail (and its
// draft composer) is visible.
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
  manageAccess,
  resolveAttachment,
  shareId,
  signedIn,
  snapshot,
}: {
  canCompose: boolean;
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
  // The draft composer lives in the rail; when the viewport shrinks below
  // the rail's breakpoint an open draft would have no cancel/submit UI.
  if (!railVisible && draft) {
    setDraft(null);
  }

  const commentsQuery = useSharedNoteComments({ enabled: signedIn, shareId });
  const createMutation = useCreateSharedNoteComment({
    shareId,
    snapshotRevision: snapshot.contentRevision,
  });
  const deleteMutation = useDeleteSharedNoteComment({ shareId });
  const comments = useMemo(
    () => collectSharedNoteComments(commentsQuery.data),
    [commentsQuery.data],
  );
  const composeEnabled =
    canCompose &&
    hasSharedNoteCollaborationAccess(commentsQuery.data?.pages[0]);

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
      resolve: resolveAttachment ?? null,
    }),
    [snapshot.attachments, resolveAttachment],
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
      (attachment) => !referenced.has(attachment.id),
    );
  }, [body, snapshot.attachments]);

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
      ...anchored.flatMap((comment) =>
        comment.range
          ? [
              {
                commentId: comment.commentId,
                from: comment.range.from,
                to: comment.range.to,
              },
            ]
          : [],
      ),
      ...(draft
        ? [{ commentId: DRAFT_COMMENT_ID, from: draft.from, to: draft.to }]
        : []),
    ]);
    scheduleLayoutMeasure();
  }, [view, comments, draft, snapshot.contentRevision, scheduleLayoutMeasure]);

  const activateComment = (commentId: string | null) => {
    setActiveCommentId(commentId);
    const currentView = viewRef.current;
    if (currentView) setActiveCommentAnchor(currentView, commentId);
    if (commentId && !window.matchMedia("(min-width: 80rem)").matches) {
      document
        .getElementById(`shared-comment-${commentId}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
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
    setDraft({ anchor: captured, from, to, top });
  };

  const submitDraft = (commentBody: string) => {
    if (!draft) return;
    const submitted = draft;
    createMutation.mutate(
      { anchor: fromCaptured(submitted.anchor), body: commentBody },
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
        resolveAttachment={resolveAttachment}
      />
    );
  }

  return (
    <div ref={attachContainer} className="relative">
      <SharedReadAttachmentsContext.Provider value={attachmentContext}>
        <NoteEditor
          className="outline-hidden"
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
        <section className="border-color-subtle mt-10 border-t pt-6">
          <h2 className="mb-3 font-mono text-sm font-medium">Attachments</h2>
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
      {/* The page shell reserves this width at xl+ via a :has() rule keyed
          on data-comment-rail, so the rail stays inside its clip bounds.
          Signed-out readers render no rail marker and keep the centered
          column. */}
      <div
        className="absolute inset-y-0 left-full ml-6 hidden w-80 xl:block"
        data-comment-rail={signedIn ? "" : undefined}
      >
        <SharedNoteCommentRail
          activeCommentId={activeCommentId}
          canDelete={(comment) => comment.isAuthor || manageAccess}
          composer={
            draft
              ? { top: screenTops.get(DRAFT_COMMENT_ID) ?? draft.top }
              : null
          }
          composerNode={
            draft ? (
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
          items={anchoredComments.filter((comment) => comment.anchor !== null)}
          onActivate={activateComment}
          onDelete={(commentId) => deleteMutation.mutate(commentId)}
          screenTops={screenTops}
        />
      </div>
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
    defaultValues: { body: "" },
    onSubmit: ({ value }) => {
      const comment = validateSharedNoteCommentBody(value.body);
      if (!comment.valid) return;
      onSubmit(comment.body);
    },
  });

  return (
    <form
      className="surface border-color-subtle rounded-2xl border p-4 shadow-md"
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
              <textarea
                autoFocus
                aria-label="Comment on selected text"
                className={cn([
                  "surface-subtle border-color-subtle text-color min-h-20 w-full resize-y rounded-xl border px-3 py-2",
                  "placeholder:text-color-muted text-sm leading-6 focus:border-stone-400 focus:ring-2 focus:ring-stone-300 focus:outline-hidden",
                ])}
                aria-invalid={tooLong}
                placeholder="Comment on the selected text…"
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
              />
              {tooLong && (
                <p className="mt-2 text-xs text-red-700" role="alert">
                  Comment is too long ({comment.byteLength.toLocaleString()}/
                  {MAX_SHARED_NOTE_COMMENT_BYTES.toLocaleString()} bytes).
                </p>
              )}
              {error && (
                <p className="mt-2 text-xs text-red-700" role="status">
                  Your comment couldn’t be added. Try again.
                </p>
              )}
              <div className="mt-3 flex justify-end gap-2">
                <button
                  type="button"
                  className={cn([
                    sharedSecondaryButtonClassName,
                    "min-h-9 px-3 text-xs",
                  ])}
                  disabled={pending}
                  onClick={onCancel}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className={cn([
                    sharedPrimaryButtonClassName,
                    "min-h-9 px-3 text-xs",
                  ])}
                  disabled={pending || !comment.valid}
                >
                  {pending && (
                    <LoaderCircleIcon
                      className="mr-1.5 size-3.5 animate-spin"
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
  const { attachments, resolve } = useContext(SharedReadAttachmentsContext);
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
      <SharedReadAttachment
        attachment={attachment}
        isImage={nodeProps.node.type.name === "image"}
        resolve={resolve}
      />
    </div>
  );
});

const readAttachmentNodeViews = {
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
    isMatchingDownload(attachment, downloadQuery.data)
      ? downloadQuery.data
      : null;

  if (
    attachment &&
    download &&
    isImage &&
    isInlineImage(attachment.contentType)
  ) {
    return (
      <figure className="my-6">
        <img
          src={download.signedUrl}
          alt={attachment.filename}
          loading="lazy"
          referrerPolicy="no-referrer"
          className="border-color-subtle max-h-[70vh] max-w-full rounded-xl border object-contain"
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
        className="surface-subtle border-color-subtle text-color my-4 flex items-center justify-between gap-4 rounded-xl border px-4 py-3 no-underline"
      >
        <span className="min-w-0 truncate font-medium">
          {attachment.filename}
        </span>
        <span className="text-color-muted shrink-0 text-xs">
          {formatFileSize(attachment.sizeBytes)}
        </span>
      </a>
    );
  }

  const Icon = isImage ? ImageIcon : FileIcon;
  return (
    <div className="border-color-subtle bg-surface-subtle my-3 flex items-center gap-3 rounded-xl border px-4 py-3">
      <div className="bg-surface flex size-10 shrink-0 items-center justify-center rounded-lg">
        <Icon className="text-color-muted size-5" aria-hidden="true" />
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-color truncate text-sm font-medium">
          {attachment?.filename ?? "Attachment unavailable"}
        </p>
        <p className="text-color-muted mt-0.5 text-xs">
          {attachment
            ? `${formatFileSize(attachment.sizeBytes)} · Included with shared note`
            : "Included attachment"}
        </p>
      </div>
    </div>
  );
}

function isMatchingDownload(
  attachment: SharedNoteAttachment,
  download: SharedNoteAttachmentDownload | null | undefined,
): download is SharedNoteAttachmentDownload {
  return Boolean(
    download &&
    download.id === attachment.id &&
    download.filename === attachment.filename &&
    download.contentType === attachment.contentType &&
    download.sizeBytes === attachment.sizeBytes &&
    download.sha256 === attachment.sha256,
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
