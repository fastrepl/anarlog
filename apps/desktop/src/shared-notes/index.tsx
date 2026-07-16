import { Trans, useLingui } from "@lingui/react/macro";
import {
  AlertCircleIcon,
  LinkIcon,
  LogInIcon,
  UsersRoundIcon,
} from "lucide-react";

import { NoteEditor } from "@hypr/editor/note";

import { useAuth } from "~/auth";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { SessionSurface } from "~/session/components/session-surface";
import { ensureFirstLineTitle } from "~/session/title-content";
import { useDurableSharedNote } from "~/shared-notes/cache";
import { useSharedNotePreview } from "~/shared-notes/preview";
import type { Tab } from "~/store/zustand/tabs";

export function TabContentSharedNote({
  tab,
}: {
  tab: Extract<Tab, { type: "shared_sessions" }>;
}) {
  const { t } = useLingui();
  const { session } = useAuth();
  const snapshotQuery = useDurableSharedNote(session?.user.id, tab.id);

  if (session === undefined || snapshotQuery.isLoading) {
    return <SharedNoteLoading />;
  }
  if (!session) {
    return (
      <SharedNoteUnavailable
        icon={LogInIcon}
        title={t`Sign in to view this shared note`}
        description={t`Shared notes are tied to the account they were shared with.`}
      />
    );
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
    <SharedNoteDocument
      body={snapshot.body}
      contentKey={`${snapshot.shareId}:${snapshot.contentRevision}`}
      icon={UsersRoundIcon}
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
  return (
    <SharedNoteDocument
      body={snapshot.body}
      contentKey={`${tab.id}:${snapshot.contentRevision}`}
      icon={LinkIcon}
      subtitle={<Trans>Shared link · View only</Trans>}
      title={snapshot.title}
    />
  );
}

function SharedNoteDocument({
  body,
  contentKey,
  icon: Icon,
  subtitle,
  title,
}: {
  body: Parameters<typeof ensureFirstLineTitle>[0];
  contentKey: string;
  icon: typeof UsersRoundIcon;
  subtitle: React.ReactNode;
  title: string;
}) {
  const { t } = useLingui();
  const content = ensureFirstLineTitle(body, title);
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
          showFormatToolbar={false}
        />
      </div>
    </SessionSurface>
  );
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
  icon: Icon,
  title,
  description,
}: {
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
        </div>
      </div>
    </SessionSurface>
  );
}
