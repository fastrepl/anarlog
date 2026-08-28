import { Trans, useLingui } from "@lingui/react/macro";
import {
  Link,
  Paperclip,
  SignIn,
  Users,
  WarningCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { NoteEditor } from "@anlg/editor/note";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import {
  SessionCommentsLayer,
  useSharedSessionComments,
} from "~/session-sharing/comments";
import { SessionSurface } from "~/session/components/session-surface";
import { ensureFirstLineTitle } from "~/session/title-content";
import {
  type SharedNoteSnapshot,
  useDurableSharedNote,
} from "~/shared-notes/cache";
import { useSharedNotePreview } from "~/shared-notes/preview";
import { useSharedAttachmentResolver } from "~/shared-notes/use-shared-attachment-resolver";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
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
        icon={SignIn}
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
        icon={WarningCircle}
        title={t`Shared note unavailable`}
        description={t`Anarlog could not read the local shared-note cache.`}
      />
    );
  }

  const snapshot = snapshotQuery.data;
  if (!snapshot) {
    return (
      <SharedNoteUnavailable
        icon={Users}
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
      sx={styles.signInButton}
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

function AuthenticatedSharedNoteDocument({
  snapshot,
  viewerUserId,
}: {
  snapshot: SharedNoteSnapshot;
  viewerUserId: string;
}) {
  const resolveAttachment = useSharedAttachmentResolver(
    viewerUserId,
    snapshot.shareId,
  );
  const subtitle = snapshot.manageAccess ? (
    <Trans>Shared note · Owner</Trans>
  ) : snapshot.capability === "editor" ? (
    <Trans>Shared with me · Can edit</Trans>
  ) : snapshot.capability === "commenter" ? (
    <Trans>Shared with me · Can comment</Trans>
  ) : (
    <Trans>Shared with me · View only</Trans>
  );
  return (
    <SharedNoteDocument
      body={snapshot.body}
      comments={{
        canCompose: snapshot.manageAccess || snapshot.capability !== "viewer",
        currentRevision: snapshot.contentRevision,
        manageAccess: snapshot.manageAccess,
        shareId: snapshot.shareId,
      }}
      contentKey={`${snapshot.shareId}:${snapshot.contentRevision}`}
      icon={Users}
      attachments={snapshot.attachments}
      resolveAttachment={resolveAttachment}
      subtitle={subtitle}
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
        icon={WarningCircle}
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
      icon={Link}
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
  contentKey,
  icon: Icon,
  subtitle,
  title,
  resolveAttachment,
}: {
  attachments?: SharedNoteSnapshot["attachments"];
  body: Parameters<typeof ensureFirstLineTitle>[0];
  comments?: {
    canCompose: boolean;
    currentRevision: number;
    manageAccess: boolean;
    shareId: string;
  };
  contentKey: string;
  icon: typeof Users;
  subtitle: React.ReactNode;
  title: string;
  resolveAttachment?: React.ComponentProps<
    typeof NoteEditor
  >["resolveAttachment"];
}) {
  const { t } = useLingui();
  const commentController = useSharedSessionComments({
    canCompose: comments?.canCompose ?? false,
    currentRevision: comments?.currentRevision ?? -1,
    manageAccess: comments?.manageAccess ?? false,
    shareId: comments?.shareId ?? null,
  });
  useMountEffect(() => {
    trackAnalyticsEvent("shared_note_opened", {
      access_mode: comments
        ? comments.manageAccess
          ? "owner"
          : comments.canCompose
            ? "collaborator"
            : "viewer"
        : "public_preview",
    });
  });

  const content = ensureFirstLineTitle(
    hydrateSharedAttachmentAttrs(body, attachments),
    title,
  );
  return (
    <SessionSurface
      header={
        <div {...stylex.props(styles.header)}>
          <Icon {...stylex.props(styles.headerIcon)} />
          <div {...stylex.props(styles.headerCopy)}>
            <div {...stylex.props(styles.headerTitle)}>
              {title || t`Untitled`}
            </div>
            <div {...stylex.props(styles.headerSubtitle)}>{subtitle}</div>
          </div>
        </div>
      }
    >
      <div {...stylex.props(styles.content)}>
        <div
          ref={commentController.containerRef}
          {...stylex.props(styles.editorContainer)}
        >
          <NoteEditor
            key={contentKey}
            className="session-note-editor"
            commentAnchorsEnabled={comments !== undefined}
            initialContent={content}
            onCommentAnchorsEvent={
              comments ? commentController.onCommentAnchorsEvent : undefined
            }
            onCommentSelection={
              commentController.selection && !commentController.draft
                ? commentController.startDraft
                : undefined
            }
            onLinkOpen={openEditorLink}
            onViewDisposed={
              comments ? commentController.onViewDisposed : undefined
            }
            onViewReady={comments ? commentController.onViewReady : undefined}
            readOnly
            resolveAttachment={resolveAttachment}
            showFormatToolbar={false}
          />
          <SessionCommentsLayer controller={commentController} />
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
    <section {...stylex.props(styles.attachments)}>
      <h2 {...stylex.props(styles.attachmentsTitle)}>Attachments</h2>
      <div {...stylex.props(styles.attachmentList)}>
        {unreferenced.map((attachment) => {
          const resolution = resolveAttachment(attachment.id);
          if (attachment.contentType.startsWith("audio/") && resolution?.src) {
            return (
              <div
                key={attachment.id}
                {...stylex.props(styles.audioAttachment)}
              >
                <p {...stylex.props(styles.audioAttachmentName)}>
                  {attachment.filename}
                </p>
                <audio
                  controls
                  preload="metadata"
                  src={resolution.src}
                  {...stylex.props(styles.audio)}
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
              {...stylex.props(styles.fileAttachment)}
            >
              <Paperclip
                {...stylex.props(styles.attachmentIcon)}
                aria-hidden="true"
              />
              <span {...stylex.props(styles.attachmentName)}>
                {attachment.filename}
              </span>
              <span {...stylex.props(styles.attachmentMeta)}>
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
      <div {...stylex.props(styles.loading)}>
        <div {...stylex.props(styles.skeleton, styles.skeletonTitle)} />
        <div {...stylex.props(styles.skeleton, styles.skeletonWide)} />
        <div {...stylex.props(styles.skeleton, styles.skeletonNarrow)} />
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
  icon: typeof Users;
  title: string;
  description: string;
}) {
  return (
    <SessionSurface>
      <div {...stylex.props(styles.unavailable)}>
        <div {...stylex.props(styles.unavailableContent)}>
          <Icon {...stylex.props(styles.unavailableIcon)} />
          <h1 {...stylex.props(styles.unavailableTitle)}>{title}</h1>
          <p {...stylex.props(styles.unavailableDescription)}>{description}</p>
          {action}
        </div>
      </div>
    </SessionSurface>
  );
}

const pulse = stylex.keyframes({
  "0%, 100%": {
    opacity: 1,
  },
  "50%": {
    opacity: 0.5,
  },
});

const styles = stylex.create({
  attachmentIcon: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  attachmentList: {
    marginTop: {
      default: null,
      ":is(*) > * + *": "0.5rem",
    },
  },
  attachmentMeta: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
  },
  attachmentName: {
    flex: "1",
    fontSize: "0.875rem",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  attachments: {
    borderColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderStyle: "solid",
    borderTopWidth: "1px",
    borderRightWidth: "0",
    borderBottomWidth: "0",
    borderLeftWidth: "0",
    marginTop: "2rem",
    paddingTop: "1.25rem",
  },
  attachmentsTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
    marginBottom: "0.5rem",
  },
  audio: {
    height: "2.25rem",
    width: "100%",
  },
  audioAttachment: {
    borderColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    paddingBlock: "0.75rem",
    paddingInline: "0.75rem",
  },
  audioAttachmentName: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginBottom: "0.5rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  content: {
    height: "100%",
    overflow: "auto",
    paddingBottom: "1.5rem",
    paddingInline: "0.75rem",
    paddingTop: "0.5rem",
  },
  editorContainer: {
    position: "relative",
  },
  fileAttachment: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.muted} 60%, transparent)`,
    },
    borderColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    color: {
      default: null,
      ":disabled": colors.mutedForeground,
    },
    cursor: {
      default: null,
      ":disabled": "not-allowed",
    },
    display: "flex",
    gap: "0.75rem",
    paddingBlock: "0.75rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    width: "100%",
  },
  header: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    height: "3rem",
    minWidth: 0,
    paddingInline: "0.75rem",
  },
  headerCopy: {
    minWidth: 0,
  },
  headerIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  headerSubtitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  headerTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  loading: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    height: "100%",
    paddingBlock: "1.25rem",
    paddingInline: "1rem",
  },
  signInButton: {
    marginTop: "1rem",
  },
  skeleton: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    borderRadius: radii.md,
  },
  skeletonNarrow: {
    backgroundColor: `color-mix(in oklab, ${colors.muted} 70%, transparent)`,
    height: "1rem",
    width: "66.666667%",
  },
  skeletonTitle: {
    backgroundColor: colors.muted,
    height: "1.25rem",
    width: "60%",
  },
  skeletonWide: {
    backgroundColor: `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
    height: "1rem",
    width: "80%",
  },
  unavailable: {
    alignItems: "center",
    display: "flex",
    height: "100%",
    justifyContent: "center",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  unavailableContent: {
    maxWidth: "24rem",
  },
  unavailableDescription: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    marginTop: "0.25rem",
  },
  unavailableIcon: {
    color: colors.mutedForeground,
    height: "1.5rem",
    marginBottom: "0.75rem",
    marginInline: "auto",
    width: "1.5rem",
  },
  unavailableTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
});
