import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  LinkIcon,
  LogInIcon,
  PaperclipIcon,
  UsersRoundIcon,
} from "lucide-react";
import type { EditorView } from "prosemirror-view";
import { useEffect, useRef, useState } from "react";

import {
  type CommentAnchorsEvent,
  NoteEditor,
  setActiveCommentAnchor,
} from "@hypr/editor/note";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@hypr/ui/components/ui/popover";
import { formatDistanceToNow } from "@hypr/utils";

import { useAuth } from "~/auth";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import {
  listSessionShareComments,
  type SessionShareComment,
  ShareManagementError,
} from "~/session-sharing/client";
import {
  applySessionShareCommentAnchors,
  sessionShareCommentsQueryKey,
} from "~/session-sharing/comment-anchors";
import { SessionSurface } from "~/session/components/session-surface";
import { ensureFirstLineTitle } from "~/session/title-content";
import {
  type SharedNoteSnapshot,
  useDurableSharedNote,
} from "~/shared-notes/cache";
import { useSharedNotePreview } from "~/shared-notes/preview";
import { useSharedAttachmentResolver } from "~/shared-notes/use-shared-attachment-resolver";
import type { Tab } from "~/store/zustand/tabs";

export function TabContentSharedNote({
  tab,
}: {
  tab: Extract<Tab, { type: "shared_sessions" }>;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const { session } = auth;
  const viewerUserId =
    session && session.user.is_anonymous !== true ? session.user.id : null;
  const snapshotQuery = useDurableSharedNote(viewerUserId, tab.id);

  if (session === undefined) {
    return <SharedNoteLoading />;
  }
  if (!viewerUserId) {
    return (
      <SharedNoteUnavailable
        action={<SharedNoteSignInAction />}
        icon={LogInIcon}
        title={t`Sign in to view this shared note`}
        description={t`Shared notes are tied to the account they were shared with.`}
      />
    );
  }
  if (snapshotQuery.isLoading) {
    return <SharedNoteLoading />;
  }
  if (snapshotQuery.error) {
    return (
      <SharedNoteUnavailable
        icon={AlertCircleIcon}
        title={t`Shared note unavailable`}
        description={t`Anarlog could not read the local shared-note cache.`}
      />
    );
  }

  const snapshot = snapshotQuery.data;
  if (!snapshot) {
    return (
      <SharedNoteUnavailable
        icon={UsersRoundIcon}
        title={t`Access no longer available`}
        description={t`The note may have been unshared or moved out of a workspace you can access.`}
      />
    );
  }

  return (
    <AuthenticatedSharedNoteDocument
      snapshot={snapshot}
      viewerUserId={viewerUserId}
    />
  );
}

function SharedNoteSignInAction() {
  const auth = useAuth();
  const signInMutation = useMutation({ mutationFn: () => auth.signIn() });

  return (
    <Button
      className="mt-4"
      disabled={signInMutation.isPending}
      onClick={() => signInMutation.mutate()}
    >
      {signInMutation.isPending ? (
        <Trans>Opening…</Trans>
      ) : (
        <Trans>Sign in</Trans>
      )}
    </Button>
  );
}

const noSharedNoteComments: SessionShareComment[] = [];

function AuthenticatedSharedNoteDocument({
  snapshot,
  viewerUserId,
}: {
  snapshot: SharedNoteSnapshot;
  viewerUserId: string;
}) {
  const auth = useAuth();
  const resolveAttachment = useSharedAttachmentResolver(
    viewerUserId,
    snapshot.shareId,
  );
  const supabase = auth.supabase;
  const session =
    auth.session && auth.session.user.is_anonymous !== true
      ? auth.session
      : null;
  // First page only in v1; older comments stay unfetched until pagination UI
  // exists.
  const commentsQuery = useQuery({
    queryKey: sessionShareCommentsQueryKey(snapshot.shareId),
    queryFn: ({ signal }) => {
      if (!supabase || !session) {
        throw new ShareManagementError();
      }
      return listSessionShareComments(
        { supabase, session, signal },
        { shareId: snapshot.shareId },
      );
    },
    enabled: supabase !== null && session !== null,
  });
  return (
    <SharedNoteDocument
      body={snapshot.body}
      comments={commentsQuery.data?.comments ?? noSharedNoteComments}
      commentsRevision={snapshot.contentRevision}
      contentKey={`${snapshot.shareId}:${snapshot.contentRevision}`}
      icon={UsersRoundIcon}
      attachments={snapshot.attachments}
      resolveAttachment={resolveAttachment}
      subtitle={<Trans>Shared with me · View only</Trans>}
      title={snapshot.title}
    />
  );
}

export function TabContentSharedNotePreview({
  tab,
}: {
  tab: Extract<Tab, { type: "shared_note_preview" }>;
}) {
  const { t } = useLingui();
  const preview = useSharedNotePreview(tab.id);

  if (preview.status === "loading") {
    return <SharedNoteLoading />;
  }
  if (preview.status === "unavailable") {
    return (
      <SharedNoteUnavailable
        icon={AlertCircleIcon}
        title={t`Shared note unavailable`}
        description={t`The link may have expired or its access may have changed.`}
      />
    );
  }

  const snapshot = preview.snapshot;
  return <PreviewSharedNoteDocument snapshot={snapshot} viewId={tab.id} />;
}

function PreviewSharedNoteDocument({
  snapshot,
  viewId,
}: {
  snapshot: Extract<
    ReturnType<typeof useSharedNotePreview>,
    { status: "ready" }
  >["snapshot"];
  viewId: string;
}) {
  const downloads = new Map(
    snapshot.attachmentDownloads.map((download) => [download.id, download]),
  );
  const resolveAttachment: React.ComponentProps<
    typeof NoteEditor
  >["resolveAttachment"] = (attachmentId) => {
    const download = downloads.get(attachmentId);
    if (!download) return null;
    return download.localPath && download.localSrc
      ? { path: download.localPath, src: download.localSrc }
      : { path: download.signedUrl, src: download.signedUrl };
  };
  return (
    <SharedNoteDocument
      attachments={snapshot.attachments}
      body={snapshot.body}
      contentKey={`${viewId}:${snapshot.contentRevision}`}
      icon={LinkIcon}
      resolveAttachment={resolveAttachment}
      subtitle={<Trans>Shared link · View only</Trans>}
      title={snapshot.title}
    />
  );
}

function SharedNoteDocument({
  attachments = [],
  body,
  comments,
  commentsRevision,
  contentKey,
  icon: Icon,
  subtitle,
  title,
  resolveAttachment,
}: {
  attachments?: SharedNoteSnapshot["attachments"];
  body: Parameters<typeof ensureFirstLineTitle>[0];
  comments?: SessionShareComment[];
  commentsRevision?: number;
  contentKey: string;
  icon: typeof UsersRoundIcon;
  subtitle: React.ReactNode;
  title: string;
  resolveAttachment?: React.ComponentProps<
    typeof NoteEditor
  >["resolveAttachment"];
}) {
  const { t } = useLingui();
  const commentsEnabled = comments !== undefined;
  const editorContainerRef = useRef<HTMLDivElement | null>(null);
  const [view, setView] = useState<EditorView | null>(null);
  const [openAnchor, setOpenAnchor] = useState<{
    commentIds: string[];
    left: number;
    top: number;
  } | null>(null);

  // External sync: highlights live in the ProseMirror comment-anchors plugin,
  // so resolved ranges are pushed into the view whenever comments change.
  useEffect(() => {
    if (!view || !comments) return;
    applySessionShareCommentAnchors(view, comments, commentsRevision ?? -1);
  }, [view, comments, commentsRevision]);

  const openComments = openAnchor
    ? (comments ?? []).filter((comment) =>
        openAnchor.commentIds.includes(comment.commentId),
      )
    : [];

  const closeCommentPopover = () => {
    setOpenAnchor(null);
    if (view) setActiveCommentAnchor(view, null);
  };

  const handleCommentAnchorsEvent = (event: CommentAnchorsEvent) => {
    if (event.type !== "anchor-click" || !view) return;
    const container = editorContainerRef.current;
    if (!container) return;
    const coords = view.coordsAtPos(event.pos);
    const containerRect = container.getBoundingClientRect();
    setOpenAnchor({
      commentIds: event.commentIds,
      left: coords.left - containerRect.left,
      top: coords.bottom - containerRect.top,
    });
    setActiveCommentAnchor(view, event.commentIds[0] ?? null);
  };

  const content = ensureFirstLineTitle(
    hydrateSharedAttachmentAttrs(body, attachments),
    title,
  );
  return (
    <SessionSurface
      header={
        <div className="flex h-12 min-w-0 items-center gap-2 px-3">
          <Icon className="text-muted-foreground size-4 shrink-0" />
          <div className="min-w-0">
            <div className="truncate text-sm font-medium">
              {title || t`Untitled`}
            </div>
            <div className="text-muted-foreground text-xs">{subtitle}</div>
          </div>
        </div>
      }
    >
      <div className="h-full overflow-auto px-3 pt-2 pb-6">
        <div ref={editorContainerRef} className="relative">
          <NoteEditor
            key={contentKey}
            className="session-note-editor"
            commentAnchorsEnabled={commentsEnabled}
            initialContent={content}
            onCommentAnchorsEvent={
              commentsEnabled ? handleCommentAnchorsEvent : undefined
            }
            onLinkOpen={openEditorLink}
            onViewDisposed={
              commentsEnabled
                ? () => {
                    setView(null);
                    setOpenAnchor(null);
                  }
                : undefined
            }
            onViewReady={commentsEnabled ? setView : undefined}
            readOnly
            resolveAttachment={resolveAttachment}
            showFormatToolbar={false}
          />
          {openAnchor && openComments.length > 0 && (
            <Popover
              open
              onOpenChange={(open) => {
                if (!open) closeCommentPopover();
              }}
            >
              <PopoverAnchor asChild>
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute size-0"
                  style={{ left: openAnchor.left, top: openAnchor.top }}
                />
              </PopoverAnchor>
              <PopoverContent
                align="start"
                side="bottom"
                className="max-h-80 w-80 overflow-y-auto p-0"
              >
                {openComments.map((comment) => (
                  <SharedNoteCommentItem
                    key={comment.commentId}
                    comment={comment}
                  />
                ))}
              </PopoverContent>
            </Popover>
          )}
        </div>
        <SharedAttachmentList
          attachments={attachments}
          body={body}
          resolveAttachment={resolveAttachment}
        />
      </div>
    </SessionSurface>
  );
}

function SharedNoteCommentItem({ comment }: { comment: SessionShareComment }) {
  return (
    <div className="border-border/60 border-b px-4 py-3 last:border-b-0">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-sm font-medium">
          {comment.isAuthor ? <Trans>You</Trans> : <Trans>Collaborator</Trans>}
        </span>
        <span className="text-muted-foreground shrink-0 text-xs">
          {formatDistanceToNow(new Date(comment.createdAt), {
            addSuffix: true,
          })}
        </span>
      </div>
      {comment.anchor && (
        <p className="text-muted-foreground border-border/60 mt-1.5 truncate border-l-2 pl-2 text-xs italic">
          {truncateCommentQuote(comment.anchor.quoteExact)}
        </p>
      )}
      <p className="mt-1.5 text-sm whitespace-pre-wrap">{comment.body}</p>
    </div>
  );
}

function truncateCommentQuote(quote: string, maxLength = 80) {
  if (quote.length <= maxLength) return quote;
  return `${quote.slice(0, maxLength - 1).trimEnd()}…`;
}

function hydrateSharedAttachmentAttrs(
  body: SharedNoteSnapshot["body"],
  attachments: SharedNoteSnapshot["attachments"],
): SharedNoteSnapshot["body"] {
  const manifest = new Map(
    attachments.map((attachment) => [attachment.id, attachment]),
  );
  const visit = (
    node: SharedNoteSnapshot["body"],
  ): SharedNoteSnapshot["body"] => {
    const content = node.content?.map(visit);
    const sharedAttachmentId = node.attrs?.sharedAttachmentId;
    const attachment =
      typeof sharedAttachmentId === "string"
        ? manifest.get(sharedAttachmentId)
        : undefined;
    if (!attachment) return content ? { ...node, content } : node;
    const attrs = { ...node.attrs };
    if (node.type === "image") {
      attrs.alt = attachment.filename;
    } else if (node.type === "fileAttachment") {
      attrs.name = attachment.filename;
      attrs.mimeType = attachment.contentType;
      attrs.size = attachment.sizeBytes;
    }
    return { ...node, attrs, ...(content ? { content } : {}) };
  };
  return visit(body);
}

function SharedAttachmentList({
  attachments,
  body,
  resolveAttachment,
}: {
  attachments: SharedNoteSnapshot["attachments"];
  body: SharedNoteSnapshot["body"];
  resolveAttachment?: React.ComponentProps<
    typeof NoteEditor
  >["resolveAttachment"];
}) {
  if (!resolveAttachment) return null;
  const referenced = collectSharedAttachmentIds(body);
  const unreferenced = attachments.filter(
    (attachment) => !referenced.has(attachment.id),
  );
  if (unreferenced.length === 0) return null;
  return (
    <section className="border-border/60 mt-8 border-t pt-5">
      <h2 className="mb-2 text-sm font-medium">Attachments</h2>
      <div className="space-y-2">
        {unreferenced.map((attachment) => {
          const resolution = resolveAttachment(attachment.id);
          if (attachment.contentType.startsWith("audio/") && resolution?.src) {
            return (
              <div
                key={attachment.id}
                className="border-border/60 rounded-xl border px-3 py-3"
              >
                <p className="text-muted-foreground mb-2 truncate text-xs">
                  {attachment.filename}
                </p>
                <audio
                  controls
                  preload="metadata"
                  src={resolution.src}
                  className="h-9 w-full"
                />
              </div>
            );
          }
          return (
            <button
              key={attachment.id}
              type="button"
              disabled={!resolution?.path}
              onClick={() => {
                if (!resolution?.path) return;
                if (resolution.path.startsWith("https://")) {
                  void openEditorLink(resolution.path);
                } else {
                  void openerCommands.openPath(resolution.path, null);
                }
              }}
              className="border-border/60 hover:bg-muted/60 disabled:text-muted-foreground flex w-full items-center gap-3 rounded-xl border px-3 py-3 text-left disabled:cursor-not-allowed"
            >
              <PaperclipIcon className="size-4 shrink-0" aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate text-sm">
                {attachment.filename}
              </span>
              <span className="text-muted-foreground shrink-0 text-xs">
                {resolution
                  ? formatFileSize(attachment.sizeBytes)
                  : "Unavailable"}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function collectSharedAttachmentIds(root: SharedNoteSnapshot["body"]) {
  const ids = new Set<string>();
  const visit = (node: typeof root) => {
    const id = node.attrs?.sharedAttachmentId;
    if (typeof id === "string") ids.add(id);
    node.content?.forEach(visit);
  };
  visit(root);
  return ids;
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function SharedNoteLoading() {
  return (
    <SessionSurface>
      <div className="flex h-full flex-col gap-3 px-4 py-5">
        <div className="bg-muted h-5 w-3/5 animate-pulse rounded-md" />
        <div className="bg-muted/80 h-4 w-4/5 animate-pulse rounded-md" />
        <div className="bg-muted/70 h-4 w-2/3 animate-pulse rounded-md" />
      </div>
    </SessionSurface>
  );
}

function SharedNoteUnavailable({
  action,
  icon: Icon,
  title,
  description,
}: {
  action?: React.ReactNode;
  icon: typeof UsersRoundIcon;
  title: string;
  description: string;
}) {
  return (
    <SessionSurface>
      <div className="flex h-full items-center justify-center px-6 text-center">
        <div className="max-w-sm">
          <Icon className="text-muted-foreground mx-auto mb-3 size-6" />
          <h1 className="text-sm font-medium">{title}</h1>
          <p className="text-muted-foreground mt-1 text-sm">{description}</p>
          {action}
        </div>
      </div>
    </SessionSurface>
  );
}
