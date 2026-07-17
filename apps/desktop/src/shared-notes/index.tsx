import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  LinkIcon,
  LogInIcon,
  PaperclipIcon,
  UsersRoundIcon,
} from "lucide-react";

import { NoteEditor } from "@hypr/editor/note";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";

import { useAuth } from "~/auth";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
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
  return (
    <SharedNoteDocument
      body={snapshot.body}
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
  contentKey,
  icon: Icon,
  subtitle,
  title,
  resolveAttachment,
}: {
  attachments?: SharedNoteSnapshot["attachments"];
  body: Parameters<typeof ensureFirstLineTitle>[0];
  contentKey: string;
  icon: typeof UsersRoundIcon;
  subtitle: React.ReactNode;
  title: string;
  resolveAttachment?: React.ComponentProps<
    typeof NoteEditor
  >["resolveAttachment"];
}) {
  const { t } = useLingui();
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
        <NoteEditor
          key={contentKey}
          className="session-note-editor"
          initialContent={content}
          onLinkOpen={openEditorLink}
          readOnly
          resolveAttachment={resolveAttachment}
          showFormatToolbar={false}
        />
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
